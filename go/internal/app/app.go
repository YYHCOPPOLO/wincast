package app

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"runtime"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

// Run owns the only UI event loop. Preview paths and identity are kept apart
// from the daily application, including when this is run outside the packager.
func Run() error {
	started := time.Now()
	p, err := previewProfile()
	if err != nil {
		return err
	}
	file, err := os.OpenFile(filepath.Join(p.logs, "tinycast.log"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
	if err != nil {
		return fmt.Errorf("open preview log: %w", err)
	}
	defer file.Close()
	log.SetOutput(file)
	log.SetFlags(log.Ldate | log.Ltime | log.Lmicroseconds | log.LUTC)
	log.Printf("startup go=%s mygo=%s profile=%s cache=%s data=%s", runtime.Version(), mygo.Version, p.settings, p.cache, p.data)
	mygo.App.SetName(previewID)
	mygo.App.SetPath(mygo.PathUserData, p.settings)
	mygo.App.SetPath(mygo.PathCache, p.cache)
	mygo.App.SetPath(mygo.PathLogs, p.logs)
	if !mygo.App.RequestSingleInstanceLock() {
		return nil
	}
	mygo.App.WhenReady(func() {
		window := mygo.NewWindow(mygo.WindowOptions{
			Title: "Tinycast Go Preview", Width: 680, Height: 420,
			Content: ui.View(func(c *ui.Context) {
				ui.Column(c).Fill().Padding(24).Gap(16).Children(func() {
					ui.Text(c, "Tinycast Go Preview").FontSize(24)
					ui.Text(c, "Go + MyGo 原生界面 · 独立预览")
					ui.Text(c, "预览数据与日用 Tinycast 隔离。")
					if ui.Button(c, "退出预览").Clicked() {
						mygo.App.Quit()
					}
				})
			}),
		})
		window.Show()
		window.Focus()
		log.Printf("ready elapsed_us=%d", time.Since(started).Microseconds())
	})
	return mygo.App.Run()
}
