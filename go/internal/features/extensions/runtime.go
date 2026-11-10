package extensions

import (
	_ "embed"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"sync"
	"sync/atomic"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/extension"
	"github.com/fastschema/qjs"
)

//go:embed runtime/RaycastRuntime.generated.js
var runtimeSource string

const bootstrapJS = `
globalThis.global = globalThis;
if (typeof globalThis.process === "undefined") {
  globalThis.process = {
    env: {}, platform: "win32", arch: "x64", cwd: () => "",
    nextTick: (fn, ...args) => Promise.resolve().then(() => fn(...args)),
    versions: { node: "22.0.0" }, pid: 1, version: "v22.0.0"
  };
}
globalThis.__tinycastHost = {
  log: (level, message) => __go_log(level, message),
  render: (session, json) => __go_render(session, json),
  failed: (session, message) => __go_failed(session, message),
  navigationDepthChanged: (session, depth) => __go_navigation(session, String(depth)),
  finished: (session) => __go_finished(session),
  fieldCommand: (command, fieldID) => __go_field(command, fieldID),
  invoke: (callID, api, method, args) => __go_invoke(callID, api, method, args),
  invokeSync: (api, method, args) => __go_invoke_sync(api, method, args),
  startTimer: (id, milliseconds, repeats) => __go_start_timer(id, milliseconds, repeats),
  clearTimer: (id) => __go_clear_timer(id),
};
globalThis.__tinycastCompile = function (code, filename) {
  return (0, eval)("(function (exports, require, module, __filename, __dirname) {\n" + code + "\n})");
};
`

type runtimeJob struct {
	fn   func()
	done chan struct{}
}

type runtimeSession struct {
	id      string
	manager *Manager
	ext     *Installed
	command extension.Command
	mode    extension.CommandMode

	runtime *qjs.Runtime
	context *qjs.Context

	jobs     chan runtimeJob
	finished chan struct{}
	stopOnce sync.Once
	stopped  atomic.Bool

	timersMu sync.Mutex
	timers   map[string]*time.Timer

	lastRender atomic.Int64
}

func newRuntimeSession(id string, manager *Manager, installed *Installed, command extension.Command) *runtimeSession {
	return &runtimeSession{
		id:       id,
		manager:  manager,
		ext:      installed,
		command:  command,
		mode:     command.Mode,
		jobs:     make(chan runtimeJob, 32),
		finished: make(chan struct{}),
		timers:   make(map[string]*time.Timer),
	}
}

func (s *runtimeSession) start() {
	go s.run()
}

func (s *runtimeSession) run() {
	defer func() {
		if s.runtime != nil {
			s.runtime.Close()
			s.runtime = nil
		}
		s.manager.sessionEnded(s)
	}()
	rt, err := qjs.New(qjs.Option{
		CWD: s.ext.Directory,
		// A Windows GUI process has no valid standard handles. Passing
		// os.Stdout/os.Stderr makes wazero call GetFileType on an invalid
		// handle and the whole extension runtime fails to instantiate.
		// Extension output reaches the app log through __go_log instead.
		Stdout: io.Discard,
		Stderr: io.Discard,
	})
	if err != nil {
		s.manager.events.Failed(s.id, fmt.Sprintf("Could not start the extension runtime: %v", err))
		return
	}
	s.runtime = rt
	s.context = rt.Context()
	if err := s.installHost(); err != nil {
		s.manager.events.Failed(s.id, err.Error())
		return
	}
	if err := s.boot(); err != nil {
		s.manager.events.Failed(s.id, err.Error())
		return
	}
	if err := s.launch(); err != nil {
		s.manager.events.Failed(s.id, err.Error())
		return
	}
	for {
		select {
		case <-s.finished:
			return
		case job := <-s.jobs:
			job.fn()
			close(job.done)
		}
	}
}

func (s *runtimeSession) boot() error {
	config, err := s.bootConfig()
	if err != nil {
		return err
	}
	if _, err := s.context.Eval("bootstrap.js", qjs.Code(bootstrapJS)); err != nil {
		return fmt.Errorf("Could not install the extension host: %w", err)
	}
	if _, err := s.context.Eval("runtime.js", qjs.Code(runtimeSource)); err != nil {
		return fmt.Errorf("Could not start the extension runtime: %w", err)
	}
	tinycast := s.context.Global().GetPropertyStr("__tinycast")
	if tinycast.IsUndefined() || tinycast.IsNull() {
		return fmt.Errorf("Could not start the extension runtime: __tinycast is missing")
	}
	value, err := tinycast.Invoke("boot", config)
	if err != nil {
		return fmt.Errorf("Could not start the extension runtime: %w", err)
	}
	value.Free()
	return nil
}

func (s *runtimeSession) launch() error {
	bundle, ok := s.ext.BundlePath(s.command)
	if !ok {
		return fmt.Errorf("%s has no built command bundle for %s", s.ext.Title(), s.command.Title)
	}
	code, err := os.ReadFile(bundle)
	if err != nil {
		return fmt.Errorf("Could not read %s: %w", filepath.Base(bundle), err)
	}
	if s.manager.events.Started != nil {
		s.manager.events.Started(s.id)
	}
	contextJSON, err := json.Marshal(s.launchContext())
	if err != nil {
		return fmt.Errorf("Could not prepare the extension launch: %w", err)
	}
	tinycast := s.context.Global().GetPropertyStr("__tinycast")
	value, err := tinycast.Invoke(
		"start",
		s.id,
		string(code),
		bundle,
		s.ext.Directory,
		s.mode.RuntimeName(),
		string(contextJSON),
	)
	if err != nil {
		return fmt.Errorf("Could not start %s: %w", s.command.Title, err)
	}
	value.Free()
	if pump, pumpErr := s.context.Eval("pump.js", qjs.Code("0")); pumpErr == nil {
		pump.Free()
	}
	return nil
}

func (s *runtimeSession) launchContext() map[string]any {
	preferences := s.manager.storage.ResolvePreferences(s.ext.ID(), s.command.Preferences)
	preferenceValues := make(map[string]any, len(preferences))
	for key, value := range preferences {
		preferenceValues[key] = value.Any()
	}
	appearance := "light"
	if s.manager.AppearanceDark() {
		appearance = "dark"
	}
	environment := map[string]any{
		"extensionName":     s.ext.ID(),
		"commandName":       s.command.Name,
		"ownerOrAuthorName": s.ext.Title(),
		"entryPointType":    "command",
		"entryPointName":    s.command.Name,
		"entryPointMode":    string(s.command.Mode),
		"assetsPath":        s.ext.AssetsPath(),
		"supportPath":       s.manager.catalog.EnsureSupportPath(s.ext.ID()),
		"isDevelopment":     false,
		"raycastVersion":    "2.0.3",
		"textSize":          "medium",
		"appearance":        appearance,
		"launchType":        "userInitiated",
		"canAccess":         false,
		"commandMode":       string(s.command.Mode),
		"theme":             appearance,
	}
	launchProps := map[string]any{
		"launchType": "userInitiated",
		"arguments":  s.command.CompleteArguments(s.manager.arguments),
	}
	return map[string]any{
		"environment": environment,
		"preferences": preferenceValues,
		"caches":      s.manager.storage.Caches(s.ext.ID()),
		"launchProps": launchProps,
	}
}

func (s *runtimeSession) bootConfig() (string, error) {
	home, _ := os.UserHomeDir()
	temporary := os.TempDir()
	environment := make(map[string]string)
	for _, entry := range os.Environ() {
		for index := 0; index < len(entry); index++ {
			if entry[index] == '=' {
				environment[entry[:index]] = entry[index+1:]
				break
			}
		}
	}
	environment["HOME"] = home
	environment["USERPROFILE"] = home
	environment["PATH"] = environment["PATH"] + ";" + s.manager.searchPath()
	config := map[string]any{
		"node": map[string]any{
			"arch":     "x64",
			"release":  "Windows 11",
			"hostname": hostname(),
			"username": environment["USERNAME"],
			"shell":    "cmd.exe",
			"homedir":  home,
			"tmpdir":   temporary,
			"cwd":      s.ext.Directory,
			"cpus":     runtime.NumCPU(),
			"totalmem": 0,
			"env":      environment,
			"execPath": "",
		},
	}
	data, err := json.Marshal(config)
	if err != nil {
		return "", err
	}
	return string(data), nil
}

func hostname() string {
	name, err := os.Hostname()
	if err != nil {
		return ""
	}
	return name
}

func (s *runtimeSession) installHost() error {
	undefined := func(this *qjs.This) (*qjs.Value, error) {
		return this.Context().NewUndefined(), nil
	}
	s.context.SetFunc("__go_log", func(this *qjs.This) (*qjs.Value, error) {
		args := this.Args()
		s.manager.events.Log(args[0].String(), args[1].String())
		return this.Context().NewUndefined(), nil
	})
	s.context.SetFunc("__go_render", func(this *qjs.This) (*qjs.Value, error) {
		args := this.Args()
		tree, err := extension.ParseTree([]byte(args[1].String()))
		if err != nil {
			s.manager.events.Log("error", fmt.Sprintf("Could not decode an extension screen: %v", err))
			return this.Context().NewUndefined(), nil
		}
		s.lastRender.Store(time.Now().UnixNano())
		s.manager.events.Render(s.id, tree)
		return this.Context().NewUndefined(), nil
	})
	s.context.SetFunc("__go_failed", func(this *qjs.This) (*qjs.Value, error) {
		args := this.Args()
		s.manager.events.Failed(s.id, args[1].String())
		return this.Context().NewUndefined(), nil
	})
	s.context.SetFunc("__go_navigation", func(this *qjs.This) (*qjs.Value, error) {
		args := this.Args()
		depth, _ := strconv.Atoi(args[1].String())
		if depth < 1 {
			depth = 1
		}
		if s.manager.events.NavigationDepth != nil {
			s.manager.events.NavigationDepth(s.id, depth)
		}
		return this.Context().NewUndefined(), nil
	})
	s.context.SetFunc("__go_finished", func(this *qjs.This) (*qjs.Value, error) {
		if s.manager.events.Finished != nil {
			s.manager.events.Finished(s.id)
		}
		return this.Context().NewUndefined(), nil
	})
	s.context.SetFunc("__go_field", func(this *qjs.This) (*qjs.Value, error) {
		args := this.Args()
		if s.manager.events.FieldCommand != nil {
			s.manager.events.FieldCommand(args[0].String(), args[1].String())
		}
		return this.Context().NewUndefined(), nil
	})
	s.context.SetFunc("__go_invoke", func(this *qjs.This) (*qjs.Value, error) {
		args := this.Args()
		callID := args[0].String()
		api := args[1].String()
		method := args[2].String()
		values := extension.ParseValues([]byte(args[3].String()))
		go s.performAsync(callID, api, method, values)
		return this.Context().NewUndefined(), nil
	})
	s.context.SetFunc("__go_invoke_sync", func(this *qjs.This) (*qjs.Value, error) {
		args := this.Args()
		api := args[0].String()
		method := args[1].String()
		values := extension.ParseValues([]byte(args[2].String()))
		payload := s.manager.performSync(api, method, values)
		return this.Context().NewString(payload), nil
	})
	s.context.SetFunc("__go_start_timer", func(this *qjs.This) (*qjs.Value, error) {
		args := this.Args()
		s.startTimer(args[0].String(), args[1].Float64(), args[2].Bool())
		return this.Context().NewUndefined(), nil
	})
	s.context.SetFunc("__go_clear_timer", func(this *qjs.This) (*qjs.Value, error) {
		s.clearTimer(this.Args()[0].String())
		return this.Context().NewUndefined(), nil
	})
	_ = undefined
	return nil
}

func (s *runtimeSession) performAsync(callID, api, method string, args []extension.Value) {
	result, err := s.manager.performCapability(s, api, method, args)
	payload := ""
	if err == nil && result != nil {
		if data, marshalErr := json.Marshal(result); marshalErr == nil {
			payload = string(data)
		}
	}
	s.post(func() {
		s.settle(callID, err == nil, payload)
	})
}

func (s *runtimeSession) settle(callID string, ok bool, payload string) {
	tinycast := s.context.Global().GetPropertyStr("__tinycast")
	if tinycast.IsUndefined() || tinycast.IsNull() {
		return
	}
	value, err := tinycast.Invoke("settle", callID, ok, payload)
	if err == nil {
		value.Free()
	}
	s.pump()
}

// pump drains QuickJS's promise job queue. The Go bindings expose no
// JS_ExecutePendingJob, but evaluating a script runs the queued jobs that
// React's scheduler and an extension's await continuations depend on.
func (s *runtimeSession) pump() {
	if s.context == nil {
		return
	}
	value, err := s.context.Eval("pump.js", qjs.Code("0"))
	if err == nil {
		value.Free()
	}
}

func (s *runtimeSession) startTimer(id string, milliseconds float64, repeats bool) {
	delay := time.Duration(milliseconds) * time.Millisecond
	if delay < 0 {
		delay = 0
	}
	s.timersMu.Lock()
	if previous := s.timers[id]; previous != nil {
		previous.Stop()
	}
	s.timers[id] = nil
	s.timersMu.Unlock()
	var schedule func()
	schedule = func() {
		timer := time.AfterFunc(delay, func() {
			s.post(func() {
				tinycast := s.context.Global().GetPropertyStr("__tinycast")
				if tinycast.IsUndefined() || tinycast.IsNull() {
					return
				}
				value, err := tinycast.Invoke("fireTimer", id)
				if err == nil {
					value.Free()
				}
				s.pump()
			})
			s.timersMu.Lock()
			_, live := s.timers[id]
			if !repeats {
				delete(s.timers, id)
			}
			s.timersMu.Unlock()
			if repeats && live {
				schedule()
			}
		})
		s.timersMu.Lock()
		if _, live := s.timers[id]; live {
			s.timers[id] = timer
		}
		s.timersMu.Unlock()
	}
	schedule()
}

func (s *runtimeSession) clearTimer(id string) {
	s.timersMu.Lock()
	defer s.timersMu.Unlock()
	if timer := s.timers[id]; timer != nil {
		timer.Stop()
	}
	delete(s.timers, id)
}

func (s *runtimeSession) post(fn func()) {
	if s.stopped.Load() {
		return
	}
	job := runtimeJob{fn: fn, done: make(chan struct{})}
	select {
	case s.jobs <- job:
	case <-s.finished:
	}
}

func (s *runtimeSession) invoke(command string, args ...any) {
	job := runtimeJob{done: make(chan struct{})}
	job.fn = func() {
		tinycast := s.context.Global().GetPropertyStr("__tinycast")
		if tinycast.IsUndefined() || tinycast.IsNull() {
			return
		}
		value, err := tinycast.Invoke(command, args...)
		if err == nil {
			value.Free()
		}
	}
	select {
	case s.jobs <- job:
		<-job.done
	case <-s.finished:
	}
}

// dispatch routes one UI event back to the React callback that produced it.
func (s *runtimeSession) dispatch(handler, argsJSON string) bool {
	if s.stopped.Load() {
		return false
	}
	job := runtimeJob{done: make(chan struct{})}
	result := false
	job.fn = func() {
		tinycast := s.context.Global().GetPropertyStr("__tinycast")
		if tinycast.IsUndefined() || tinycast.IsNull() {
			return
		}
		value, err := tinycast.Invoke("dispatch", s.id, handler, argsJSON)
		if err != nil {
			s.manager.events.Log("error", fmt.Sprintf("Extension callback failed: %v", err))
			return
		}
		result = value.String() == "1"
		value.Free()
		s.pump()
	}
	select {
	case s.jobs <- job:
		<-job.done
		return result
	case <-s.finished:
		return false
	}
}

// popNavigation pops one pushed screen; false when the command is at its root.
func (s *runtimeSession) popNavigation() bool {
	if s.stopped.Load() {
		return false
	}
	job := runtimeJob{done: make(chan struct{})}
	result := false
	job.fn = func() {
		tinycast := s.context.Global().GetPropertyStr("__tinycast")
		if tinycast.IsUndefined() || tinycast.IsNull() {
			return
		}
		value, err := tinycast.Invoke("popNavigation", s.id)
		if err != nil {
			return
		}
		result = value.String() == "1"
		value.Free()
		s.pump()
	}
	select {
	case s.jobs <- job:
		<-job.done
		return result
	case <-s.finished:
		return false
	}
}

func (s *runtimeSession) stop() {
	s.stopOnce.Do(func() {
		s.stopped.Store(true)
		job := runtimeJob{done: make(chan struct{})}
		job.fn = func() {
			tinycast := s.context.Global().GetPropertyStr("__tinycast")
			if !tinycast.IsUndefined() && !tinycast.IsNull() {
				value, err := tinycast.Invoke("stop", s.id)
				if err == nil {
					value.Free()
				}
			}
			close(s.finished)
		}
		select {
		case s.jobs <- job:
		case <-s.finished:
			return
		}
		s.timersMu.Lock()
		for _, timer := range s.timers {
			if timer != nil {
				timer.Stop()
			}
		}
		s.timers = make(map[string]*time.Timer)
		s.timersMu.Unlock()
	})
}

// flushStores writes extension storage before shutdown.
func (s *runtimeSession) flushStores() {
	s.manager.storage.Flush()
}
