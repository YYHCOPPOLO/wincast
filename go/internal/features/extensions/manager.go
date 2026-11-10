package extensions

import (
	"encoding/base64"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/extension"
)

// Host is the application half of the extension bridge. The runtime package
// owns storage, the JavaScript engine and the Node shims; anything that touches
// a window, the clipboard, the shell or OAuth belongs to the host.
type Host interface {
	Perform(api, method string, arguments []extension.Value) (any, error)
}

// Events are the runtime's calls back into the palette. They may arrive on the
// JavaScript goroutine; the app marshals them onto the UI thread.
type Events struct {
	Render          func(session string, tree *extension.Tree)
	Failed          func(session, message string)
	Finished        func(session string)
	Started         func(session string)
	NavigationDepth func(session string, depth int)
	Log             func(level, message string)
	FieldCommand    func(command, fieldID string)
	DidUninstall    func(extensionName string, entryIDs []string)
}

// CommandEntry is one launcher row an extension contributes.
type CommandEntry struct {
	EntryID   string
	Extension *Installed
	Command   extension.Command
}

// Manager owns the installed set and the single running command.
type Manager struct {
	mu        sync.Mutex
	catalog   *Catalog
	storage   *Storage
	host      Host
	events    Events
	enabled   bool
	showRows  bool
	dark      bool
	installed []*Installed

	active    *runtimeSession
	arguments map[string]string
	nextID    atomic.Uint64
	installMu sync.Mutex

	started time.Time
}

// NewManager builds the extension subsystem over the application's settings
// directory.
func NewManager(settingsDirectory string, host Host, events Events) *Manager {
	catalog := NewCatalog(settingsDirectory)
	manager := &Manager{
		catalog: catalog,
		storage: NewStorage(catalog.DataRoot()),
		host:    host,
		events:  events,
		started: time.Now(),
	}
	return manager
}

// Catalog exposes install and uninstall operations to the settings pane.
func (m *Manager) Catalog() *Catalog { return m.catalog }

// Storage exposes per-extension values to the settings pane.
func (m *Manager) Storage() *Storage { return m.storage }

// Enabled reports the consent switch.
func (m *Manager) Enabled() bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.enabled
}

// SetEnabled turns the subsystem on or off. Off means off: the running engine
// is stopped, the installed set is released and nothing is scanned.
func (m *Manager) SetEnabled(enabled bool) {
	m.mu.Lock()
	m.enabled = enabled
	session := m.active
	m.active = nil
	if !enabled {
		m.installed = nil
	}
	m.mu.Unlock()
	if session != nil {
		session.stop()
	}
	if enabled {
		m.Refresh()
	}
}

// ShowInLauncher reports whether commands are published to launcher search.
func (m *Manager) ShowInLauncher() bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.showRows
}

// SetShowInLauncher changes launcher publication without unloading anything.
func (m *Manager) SetShowInLauncher(show bool) {
	m.mu.Lock()
	m.showRows = show
	m.mu.Unlock()
}

// SetAppearanceDark records the appearance a newly launched command receives.
func (m *Manager) SetAppearanceDark(dark bool) {
	m.mu.Lock()
	m.dark = dark
	m.mu.Unlock()
}

// AppearanceDark reports the appearance injected into the next launch.
func (m *Manager) AppearanceDark() bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.dark
}

// Refresh rescans the installed set. It is a no-op while extensions are off.
func (m *Manager) Refresh() {
	if !m.Enabled() {
		return
	}
	installed := m.catalog.Scan()
	m.mu.Lock()
	m.installed = installed
	m.mu.Unlock()
}

// Installed returns the current installed set.
func (m *Manager) Installed() []*Installed {
	m.mu.Lock()
	defer m.mu.Unlock()
	return append([]*Installed(nil), m.installed...)
}

// Commands lists every installed command, including menu-bar commands and
// commands hidden from launcher search: activation explains why they cannot run.
func (m *Manager) Commands() []CommandEntry {
	installed := m.Installed()
	entries := make([]CommandEntry, 0)
	for _, owner := range installed {
		for _, command := range owner.Manifest.Commands {
			entries = append(entries, CommandEntry{
				EntryID:   CommandRef{ExtensionName: owner.ID(), CommandName: command.Name}.EntryID(),
				Extension: owner,
				Command:   command,
			})
		}
	}
	return entries
}

// Resolve finds a launcher entry.
func (m *Manager) Resolve(entryID string) (*Installed, extension.Command, bool) {
	reference, ok := ParseEntryID(entryID)
	if !ok {
		return nil, extension.Command{}, false
	}
	for _, owner := range m.Installed() {
		if owner.ID() != reference.ExtensionName {
			continue
		}
		if command, ok := owner.Command(reference.CommandName); ok {
			return owner, command, true
		}
	}
	return nil, extension.Command{}, false
}

// Start launches one command. It returns the session id immediately; the
// runtime boots on its own goroutine and reports through Events.
func (m *Manager) Start(entryID string, arguments map[string]string) (string, error) {
	if !m.Enabled() {
		return "", errors.New("Extensions are turned off.")
	}
	owner, command, ok := m.Resolve(entryID)
	if !ok {
		return "", fmt.Errorf("The extension command %s is not installed.", entryID)
	}
	if !command.Mode.Supported() {
		return "", errors.New(command.Mode.UnsupportedReason())
	}
	schemas := make([]extension.Preference, 0, len(owner.Manifest.Preferences)+len(command.Preferences))
	schemas = append(schemas, owner.Manifest.Preferences...)
	schemas = append(schemas, command.Preferences...)
	if missing := m.storage.MissingRequiredPreferences(owner.ID(), schemas); len(missing) > 0 {
		return "", fmt.Errorf("Set the required preference “%s” before running this command.", missing[0].DisplayTitle())
	}
	if _, ok := owner.BundlePath(command); !ok {
		return "", fmt.Errorf("%s has no built command bundle for %s.", owner.Title(), command.Title)
	}
	m.mu.Lock()
	previous := m.active
	m.active = nil
	m.arguments = arguments
	m.mu.Unlock()
	if previous != nil {
		previous.stop()
	}
	id := fmt.Sprintf("extension-%d", m.nextID.Add(1))
	session := newRuntimeSession(id, m, owner, command)
	m.mu.Lock()
	m.active = session
	m.mu.Unlock()
	session.start()
	return id, nil
}

// Active returns the running session id, or "".
func (m *Manager) Active() string {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.active == nil {
		return ""
	}
	return m.active.id
}

// ActiveExtensionName is the namespace for storage and OAuth while a command
// runs.
func (m *Manager) ActiveExtensionName() string {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.active == nil {
		return ""
	}
	return m.active.ext.ID()
}

// ActiveCommandName is the command the running session launched.
func (m *Manager) ActiveCommandName() string {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.active == nil {
		return ""
	}
	return m.active.command.Name
}

// Dispatch routes a scene handler back into the extension.
func (m *Manager) Dispatch(session, handler, argsJSON string) bool {
	m.mu.Lock()
	active := m.active
	m.mu.Unlock()
	if active == nil || active.id != session {
		return false
	}
	return active.dispatch(handler, argsJSON)
}

// PopNavigation pops one pushed screen.
func (m *Manager) PopNavigation(session string) bool {
	m.mu.Lock()
	active := m.active
	m.mu.Unlock()
	if active == nil || active.id != session {
		return false
	}
	return active.popNavigation()
}

// Stop ends the active command and releases its JavaScript engine.
func (m *Manager) Stop() {
	m.mu.Lock()
	active := m.active
	m.active = nil
	m.mu.Unlock()
	if active != nil {
		active.stop()
	}
	m.storage.Flush()
}

// InstallFolder copies an extension folder and refreshes the installed set.
func (m *Manager) InstallFolder(directory string) (*Installed, error) {
	m.installMu.Lock()
	defer m.installMu.Unlock()
	installed, err := m.catalog.Install(directory)
	if err != nil {
		return nil, err
	}
	m.Refresh()
	return installed, nil
}

// ImportRaycast copies every candidate from the local Raycast install.
func (m *Manager) ImportRaycast() ([]*Installed, []string) {
	m.installMu.Lock()
	defer m.installMu.Unlock()
	imported, failures := m.catalog.ImportRaycast()
	m.Refresh()
	return imported, failures
}

// Uninstall removes an extension and everything keyed to it.
func (m *Manager) Uninstall(owner *Installed) error {
	m.installMu.Lock()
	defer m.installMu.Unlock()
	if m.Active() != "" {
		m.Stop()
	}
	entryIDs := make([]string, 0, len(owner.Manifest.Commands))
	for _, command := range owner.Manifest.Commands {
		entryIDs = append(entryIDs, CommandRef{ExtensionName: owner.ID(), CommandName: command.Name}.EntryID())
	}
	if err := m.catalog.Uninstall(owner); err != nil {
		return err
	}
	m.storage.RemoveAll(owner.ID())
	if m.events.DidUninstall != nil {
		m.events.DidUninstall(owner.ID(), entryIDs)
	}
	m.Refresh()
	return nil
}

// sessionEnded releases the active pointer when the engine goroutine exits.
func (m *Manager) sessionEnded(session *runtimeSession) {
	m.mu.Lock()
	if m.active == session {
		m.active = nil
	}
	m.mu.Unlock()
	session.flushStores()
}

// searchPath is the PATH an extension's child processes receive. A GUI app
// inherits a bare environment; Node and the package managers live in places
// a login shell would add.
func (m *Manager) searchPath() string {
	home, _ := os.UserHomeDir()
	parts := []string{
		filepath.Join(home, "AppData", "Roaming", "npm"),
		filepath.Join(home, "AppData", "Local", "pnpm"),
		filepath.Join(home, "AppData", "Local", "Yarn", "bin"),
		filepath.Join(home, "AppData", "Local", "fnm"),
		filepath.Join(home, "AppData", "Local", "Volta", "bin"),
		filepath.Join(home, ".bun", "bin"),
		`C:\Program Files\nodejs`,
		`C:\Program Files\Git\cmd`,
	}
	parts = append(parts, strings.Split(os.Getenv("PATH"), ";")...)
	return strings.Join(parts, ";")
}

// performCapability answers one Blast capability name. Storage is the
// runtime's own; everything else goes to the application host.
func (m *Manager) performCapability(session *runtimeSession, api, method string, arguments []extension.Value) (any, error) {
	switch api {
	case "storage":
		return m.performStorage(session, method, arguments)
	case "fetch":
		if method == "request" {
			return fetchRequest(arguments)
		}
	case "proc":
		if method == "run" {
			return runProcess(arguments, true)
		}
	}
	if m.host == nil {
		return nil, fmt.Errorf("The host does not implement %s.%s.", api, method)
	}
	return m.host.Perform(api, method, arguments)
}

func (m *Manager) performStorage(session *runtimeSession, method string, arguments []extension.Value) (any, error) {
	name := session.ext.ID()
	scope := argumentString(arguments, 0, "scope")
	key := argumentString(arguments, 0, "key")
	value, hasValue := argumentValue(arguments, 0, "value")
	internal := scope == "blast:internal"
	switch method {
	case "get":
		if internal {
			value, ok := m.storage.InternalGet(name, key)
			if !ok {
				return nil, nil
			}
			return value, nil
		}
		value := m.storage.LocalGet(name, key)
		return value, nil
	case "set":
		if internal {
			if !hasValue {
				m.storage.InternalRemove(name, key)
				return nil, nil
			}
			m.storage.InternalSet(name, key, jsonScalar(value))
			return nil, nil
		}
		if !hasValue {
			return nil, nil
		}
		m.storage.LocalSet(name, key, jsonScalar(value))
		return nil, nil
	case "remove":
		if internal {
			m.storage.InternalRemove(name, key)
		} else {
			m.storage.LocalRemove(name, key)
		}
		return nil, nil
	case "all":
		return m.storage.LocalAll(name), nil
	case "clear":
		m.storage.LocalClear(name)
		return nil, nil
	}
	return nil, fmt.Errorf("Unknown storage call %s.", method)
}

func jsonScalar(value extension.Value) any {
	switch value.Kind {
	case extension.ValueString:
		return value.String
	case extension.ValueNumber:
		return value.Number
	case extension.ValueBool:
		return value.Bool
	default:
		return nil
	}
}

func argumentString(arguments []extension.Value, index int, key string) string {
	if index >= len(arguments) {
		return ""
	}
	object, ok := arguments[index].ObjectValue()
	if !ok {
		return ""
	}
	value, _ := object[key].StringValue()
	return value
}

func argumentBool(arguments []extension.Value, index int, key string) bool {
	if index >= len(arguments) {
		return false
	}
	object, ok := arguments[index].ObjectValue()
	if !ok {
		return false
	}
	value, _ := object[key].BoolValue()
	return value
}

func argumentNumber(arguments []extension.Value, index int, key string) (float64, bool) {
	if index >= len(arguments) {
		return 0, false
	}
	object, ok := arguments[index].ObjectValue()
	if !ok {
		return 0, false
	}
	return object[key].DoubleValue()
}

func argumentValue(arguments []extension.Value, index int, key string) (extension.Value, bool) {
	if index >= len(arguments) {
		return extension.NullValue, false
	}
	object, ok := arguments[index].ObjectValue()
	if !ok {
		return extension.NullValue, false
	}
	value, ok := object[key]
	return value, ok
}

func argumentObject(arguments []extension.Value, index int) map[string]extension.Value {
	if index >= len(arguments) {
		return nil
	}
	object, _ := arguments[index].ObjectValue()
	return object
}

type fetchSpec struct {
	URL        string            `json:"url"`
	Method     string            `json:"method"`
	Headers    map[string]string `json:"headers"`
	BodyBase64 string            `json:"bodyBase64"`
}

func fetchRequest(arguments []extension.Value) (any, error) {
	object := argumentObject(arguments, 0)
	spec := fetchSpec{}
	if object != nil {
		spec.URL, _ = object["url"].StringValue()
		spec.Method, _ = object["method"].StringValue()
		spec.BodyBase64, _ = object["bodyBase64"].StringValue()
		if headers, ok := object["headers"].ObjectValue(); ok {
			spec.Headers = make(map[string]string, len(headers))
			for name, value := range headers {
				text, _ := value.StringValue()
				spec.Headers[name] = text
			}
		}
	}
	if !strings.Contains(spec.URL, "://") {
		return nil, fmt.Errorf("Invalid URL: %s", spec.URL)
	}
	method := spec.Method
	if method == "" {
		method = http.MethodGet
	}
	var body []byte
	if spec.BodyBase64 != "" {
		decoded, err := base64.StdEncoding.DecodeString(spec.BodyBase64)
		if err == nil {
			body = decoded
		}
	}
	request, err := http.NewRequest(method, spec.URL, strings.NewReader(string(body)))
	if err != nil {
		return nil, fmt.Errorf("Invalid URL: %s", spec.URL)
	}
	for name, value := range spec.Headers {
		request.Header.Set(name, value)
	}
	client := &http.Client{Timeout: 60 * time.Second}
	response, err := client.Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	data, err := io.ReadAll(response.Body)
	if err != nil {
		return nil, err
	}
	headers := make(map[string]string, len(response.Header))
	for name, values := range response.Header {
		headers[strings.ToLower(name)] = strings.Join(values, ", ")
	}
	return map[string]any{
		"status":     response.StatusCode,
		"statusText": http.StatusText(response.StatusCode),
		"headers":    headers,
		"url":        response.Request.URL.String(),
		"bodyBase64": base64.StdEncoding.EncodeToString(data),
	}, nil
}
