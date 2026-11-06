package app

import (
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
	instance, first, err := windows.AcquireInstance(previewID)
	if err != nil {
		return err
	}
	defer instance.Close()
	if !first {
		// MyGo forwards this command line to the existing UI. The kernel
		// object above still prevents a second writer if IPC is unavailable.
		if mygo.App.RequestSingleInstanceLock() {
			return fmt.Errorf("Tinycast Go Preview is already running but could not be activated")
		}
		return nil
	}
	if slices.Contains(os.Args[1:], "--quit") {
		return nil
	}
	if !mygo.App.RequestSingleInstanceLock() {
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
	defer c.finish()
	mygo.App.SetActivationPolicy(mygo.ActivationPolicyAccessory)
	mygo.App.OnWindowAllClosed(func() {})
	mygo.App.OnSecondInstance(func(args []string, _ string) {
		if slices.Contains(args, "--quit") {
			mygo.App.Quit()
			return
		}
		if slices.Contains(args, "--diagnostics") {
			c.openDiagnostics()
			return
		}
		c.show()
	})
	mygo.App.OnBeforeQuit(func(*mygo.QuitEvent) { c.stop() })
	mygo.App.WhenReady(c.ready)
	return mygo.App.Run()
}
