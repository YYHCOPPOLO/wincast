package app

import (
	"context"
	"fmt"
	"log"
	"os"
	"runtime"
	"slices"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/egoist/mygo"
)

// Run owns the only UI event loop. Preview paths and identity are kept apart
// from the daily application, including when this is run outside the packager.
func Run() (runErr error) {
	started := time.Now()
	p, err := previewProfile()
	if err != nil {
		return err
	}
	mygo.App.SetName(previewID)
	mygo.App.SetPath(mygo.PathUserData, p.settings)
	mygo.App.SetPath(mygo.PathCache, p.cache)
	mygo.App.SetPath(mygo.PathLogs, p.logs)
	command := windows.ActivatePanel
	if slices.Contains(os.Args[1:], "--quit") {
		command = windows.ActivateQuit
	} else if slices.Contains(os.Args[1:], "--diagnostics") {
		command = windows.ActivateDiagnostics
	}
	instance, first, err := windows.AcquireInstance(previewID, p.cache)
	if err != nil {
		return err
	}
	if !first {
		ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
		activationErr := instance.Forward(ctx, command)
		cancel()
		instance.Close()
		if activationErr == nil {
			return nil
		}
		// Release our reference before rechecking identity: if the first
		// process exited during startup, its kernel object can now disappear.
		// A still-running owner never permits a second writer or IPC listener.
		instance, first, err = windows.AcquireInstance(previewID, p.cache)
		if err != nil {
			return err
		}
		if !first {
			instance.Close()
			return activationErr
		}
	}
	defer instance.Close()
	if command == windows.ActivateQuit {
		return nil
	}
	file, err := openLog(p.logs)
	if err != nil {
		return fmt.Errorf("open preview log: %w", err)
	}
	log.SetOutput(file)
	log.SetFlags(log.Ldate | log.Ltime | log.Lmicroseconds | log.LUTC)
	defer func() {
		if runErr != nil {
			log.Printf("application stopped with error: %v", runErr)
		}
		log.SetOutput(os.Stderr)
		file.Close()
	}()
	log.Printf("startup go=%s mygo=%s profile=%s cache=%s data=%s", runtime.Version(), mygo.Version, p.settings, p.cache, p.data)
	c := newController(started, p)
	c.instance = instance
	defer c.finish()
	mygo.App.SetActivationPolicy(mygo.ActivationPolicyAccessory)
	mygo.App.OnWindowAllClosed(func() {})
	mygo.App.OnBeforeQuit(func(*mygo.QuitEvent) { c.stop() })
	mygo.App.WhenReady(func() {
		c.ready()
		if err := instance.Listen(c.ctx, func(command windows.Activation) {
			c.panel.Update(func() {
				if c.closing {
					return
				}
				switch command {
				case windows.ActivateQuit:
					mygo.App.Quit()
				case windows.ActivateDiagnostics:
					c.openDiagnostics()
				case windows.ActivatePanel:
					c.show()
				}
			})
		}); err != nil {
			runErr = err
			mygo.App.Quit()
		}
	})
	if err := mygo.App.Run(); err != nil {
		return err
	}
	return runErr
}
