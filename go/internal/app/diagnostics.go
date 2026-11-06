package app

import (
	"fmt"
	"log"
	"os"
	"runtime"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/diagnostics"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

func (c *controller) openDiagnostics() {
	if c.closing {
		return
	}
	c.refreshDiagnostics()
	if c.diagnostic == nil {
		c.diagnostic = mygo.NewWindow(mygo.WindowOptions{
			Title: "Tinycast Go Preview — Diagnostics", Width: 760, Height: 640,
			Hidden: true, Content: ui.View(c.diagnosticsView),
		})
		c.diagnostic.OnClose(func(e *mygo.CloseEvent) {
			if !c.closing {
				e.PreventDefault()
				c.diagnostic.Hide()
			}
		})
	}
	c.diagnostic.Show()
	c.diagnostic.Focus()
}

func (c *controller) refreshDiagnostics() {
	snapshot, err := diagnostics.ReadProcess()
	if err != nil {
		log.Printf("process diagnostics failed: %v", err)
		return
	}
	if !c.snapshot.At.IsZero() {
		elapsed := snapshot.At.Sub(c.snapshot.At)
		if elapsed > 0 {
			c.cpuPercent = float64(snapshot.CPU-c.snapshot.CPU) / float64(elapsed) * 100
		}
	}
	c.snapshot = snapshot
	if exe, err := os.Executable(); err == nil {
		if info, err := os.Stat(exe); err == nil {
			c.exeBytes = info.Size()
		}
	}
	log.Printf("process ws_bytes=%d private_bytes=%d heap_bytes=%d handles=%d goroutines=%d cpu_total_ms=%d", snapshot.WorkingSet, snapshot.PrivateBytes, snapshot.HeapBytes, snapshot.Handles, snapshot.Goroutines, snapshot.CPU.Milliseconds())
}

func (c *controller) diagnosticsView(ctx *ui.Context) {
	t := ctx.Theme()
	ui.Column(ctx).Fill().Children(func() {
		ui.Row(ctx).Padding(20).Gap(10).AlignItems(ui.Center).Children(func() {
			ui.Text(ctx, c.view.Text("Tinycast 预览诊断", "Tinycast Preview Diagnostics")).FontSize(22).Grow(1).Role(ui.RoleHeading)
			if ui.Button(ctx, c.view.Text("打开搜索面板", "Open search panel")).Clicked() {
				c.show()
			}
			if ui.Button(ctx, c.view.Text("刷新", "Refresh")).Clicked() {
				c.refreshDiagnostics()
			}
		})
		ui.Divider(ctx)
		ui.Scroll(ctx).Grow(1).Children(func() {
			ui.Column(ctx).Padding(20).Gap(12).FillWidth().Selectable().Children(func() {
				row := func(label, value string) {
					ui.Column(ctx).Gap(3).Children(func() {
						ui.Text(ctx, label).FontSize(12).TextColor(t.TextMuted)
						ui.Text(ctx, value)
					})
				}
				row(c.view.Text("版本", "Versions"), fmt.Sprintf("%s · MyGo %s · %s/%s", runtime.Version(), mygo.Version, runtime.GOOS, runtime.GOARCH))
				row(c.view.Text("采样", "Sample"), c.snapshot.At.Format(time.RFC3339))
				row(c.view.Text("进程资源", "Process resources"), fmt.Sprintf("WS %s · Private %s · Heap %s\nHandles %d · Goroutines %d · CPU %.3f%% (one core, since last sample)", diagnostics.MiB(c.snapshot.WorkingSet), diagnostics.MiB(c.snapshot.PrivateBytes), diagnostics.MiB(c.snapshot.HeapBytes), c.snapshot.Handles, c.snapshot.Goroutines, c.cpuPercent))
				row(c.view.Text("程序大小", "Executable size"), fmt.Sprintf("%d bytes", c.exeBytes))
				for _, display := range c.displaySummary {
					row(c.view.Text("显示器", "Display"), display)
				}
				row(c.view.Text("应用索引", "Application index"), fmt.Sprintf("%s · %d applications", c.storeStatus, c.catalogInfo.Count))
				row(c.view.Text("词典", "Dictionary"), c.dictionaryStatus)
				row(c.view.Text("词典查询", "Dictionary lookup"), fmt.Sprintf("last SQL %dµs · canceled requests %d", c.lookupElapsed.Microseconds(), c.lookupCanceled))
				row(c.view.Text("预览设置", "Preview settings"), c.profile.settings)
				row(c.view.Text("预览缓存", "Preview cache"), c.profile.cache)
				row(c.view.Text("日志", "Logs"), c.profile.logs)
			})
		})
		ui.Divider(ctx)
		ui.Row(ctx).Padding(12, 20).Gap(12).Children(func() {
			ui.Text(ctx, c.view.Text("Alt+Shift+Space 唤出 · Tab 切换词典", "Alt+Shift+Space opens the panel · Tab switches dictionary")).FontSize(12).TextColor(t.TextMuted).Grow(1)
			if ui.Button(ctx, c.view.Text("退出预览", "Quit Preview")).Clicked() {
				mygo.App.Quit()
			}
		})
	})
}
