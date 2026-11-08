package app

import (
	"errors"
	"fmt"
	"log"
	"sort"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	domainuninstall "github.com/YYHCOPPOLO/wincast/go/internal/domain/uninstall"
	uninstallfeature "github.com/YYHCOPPOLO/wincast/go/internal/features/uninstall"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/egoist/mygo"
)

func (c *controller) openUninstallSelected() {
	application, ok := c.selected()
	if !ok || application.Kind != domain.KindApplication {
		return
	}
	c.openUninstall(application)
}

func (c *controller) openUninstall(application domain.Application) {
	if c.closing || c.uninstall == nil {
		return
	}
	c.uninstallTarget = application
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
	}
	c.view.SetModeUninstall(application.Name)
	c.showPanel(false)
	c.scanUninstall()
}

func (c *controller) scanUninstall() {
	if c.closing || c.uninstall == nil {
		return
	}
	application := c.uninstallTarget
	revision := c.uninstallRevision + 1
	c.uninstallRevision = revision
	if c.view.Mode == palette.ModeUninstall {
		c.view.UninstallLoading = true
	}
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		snapshot, err := c.uninstall.Scan(c.ctx, application.ID, application.Name, application.Path)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing || revision != c.uninstallRevision {
				return
			}
			switch {
			case errors.Is(err, uninstallfeature.ErrSelf):
				c.view.SetUninstallResults(nil, c.text("Tinycast 不能卸载自身", "Tinycast cannot uninstall itself"))
			case err != nil:
				log.Printf("uninstall scan failed: %v", err)
				c.view.SetUninstallResults(nil, c.text("查找应用文件失败", "Could not find application files"))
			default:
				c.view.SetUninstallResults(snapshot.Candidates, "")
				log.Printf("uninstall scan name=%q candidates=%d", snapshot.Target, len(snapshot.Candidates))
			}
		})
	}()
}

func (c *controller) selectedUninstall() (domainuninstall.Candidate, bool) {
	if c.view == nil || c.view.Mode != palette.ModeUninstall {
		return domainuninstall.Candidate{}, false
	}
	if c.view.Chosen < 0 || c.view.Chosen >= len(c.view.UninstallItems) {
		return domainuninstall.Candidate{}, false
	}
	return c.view.UninstallItems[c.view.Chosen], true
}

func (c *controller) confirmUninstall() {
	if c.closing || c.uninstallConfirm || c.uninstallBusy {
		return
	}
	items := c.view.UninstallSelection()
	if len(items) == 0 {
		c.view.PendingToast = c.text("没有选中要卸载的项目", "No items are selected")
		return
	}
	application := c.uninstallTarget
	c.uninstallConfirm = true
	wasVisible := c.panel.IsVisible()
	c.hide()
	message := fmt.Sprintf(c.text("%d 个项目将被移到回收站，可随时还原。", "%d items will be moved to the Recycle Bin, where you can restore them."), len(items))
	if includesInstall(items) {
		message += c.text(" 应用目录移除后，它的收藏、隐藏状态、别名和排序记录也会清除。",
			" When the install folder is removed, its favorite, hidden state, alias and ranking are cleared too.")
	}
	detail := c.uninstallDetail(items)
	options := mygo.MessageOptions{
		Parent:        c.panel,
		Type:          mygo.MessageWarning,
		Title:         fmt.Sprintf(c.text("卸载“%s”？", "Uninstall “%s”?"), application.Name),
		Message:       message,
		Detail:        detail,
		Buttons:       []string{c.text("取消", "Cancel"), c.text("移到回收站", "Move to Recycle Bin")},
		DefaultButton: 0, CancelButton: 0,
	}
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		result, err := mygo.Dialog.Message(options)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			c.uninstallConfirm = false
			if c.closing {
				return
			}
			if err != nil {
				log.Printf("uninstall confirmation failed: %v", err)
				if wasVisible {
					c.showPanel(false)
				}
				return
			}
			if result.Button != 1 {
				if wasVisible {
					c.showPanel(false)
				}
				return
			}
			c.performUninstall(items)
		})
	}()
}

func includesInstall(items []domainuninstall.Candidate) bool {
	for _, item := range items {
		if item.Kind == domainuninstall.KindInstall {
			return true
		}
	}
	return false
}

func (c *controller) uninstallDetail(items []domainuninstall.Candidate) string {
	const limit = 6
	var lines []string
	for index, item := range items {
		if index == limit {
			lines = append(lines, fmt.Sprintf(c.text("… 另有 %d 项", "… %d more"), len(items)-limit))
			break
		}
		lines = append(lines, item.DisplayPath)
	}
	return strings.Join(lines, "\n")
}

func (c *controller) performUninstall(items []domainuninstall.Candidate) {
	if c.closing || c.uninstallBusy {
		return
	}
	paths := make([]string, 0, len(items))
	for _, item := range items {
		paths = append(paths, item.Path)
	}
	c.uninstallBusy = true
	c.view.Status = c.text("正在移到回收站…", "Moving to the Recycle Bin…")
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		results := windows.RecyclePaths(paths)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			c.uninstallBusy = false
			if c.closing {
				return
			}
			failures := make(map[string]string, len(results))
			failureIDs := make(map[string]string, len(results))
			moved := make(map[string]bool, len(results))
			for _, result := range results {
				id := domainuninstall.Fold(result.Path)
				if result.Err != nil {
					failures[result.Path] = result.Err.Error()
					failureIDs[id] = result.Err.Error()
				} else {
					moved[id] = true
				}
			}
			installMoved := false
			for _, item := range items {
				if item.Kind == domainuninstall.KindInstall && moved[domainuninstall.Fold(item.Path)] {
					installMoved = true
				}
			}
			remaining := make([]domainuninstall.Candidate, 0, len(c.view.UninstallAll))
			for _, item := range c.view.UninstallAll {
				if moved[item.ID] {
					continue
				}
				if reason, failed := failureIDs[item.ID]; failed {
					item.Selected = false
					item.Removable = false
					item.LockReason = shortenUninstallError(reason)
					item.Error = item.LockReason
				}
				remaining = append(remaining, item)
			}
			c.view.SetUninstallResults(remaining, "")
			if installMoved {
				c.removeLauncherReferences(c.uninstallTarget)
			}
			c.showPanel(false)
			if len(failures) == 0 {
				c.view.Status = fmt.Sprintf(c.text("已移到回收站 %d 项", "Moved %d items to the Recycle Bin"), len(moved))
				return
			}
			c.view.Status = fmt.Sprintf(c.text("已移动 %d 项，%d 项失败", "Moved %d items; %d failed"), len(moved), len(failures))
			log.Printf("uninstall moved=%d failed=%d", len(moved), len(failures))
			c.showFailure(c.text("部分项目未能移动", "Some items were not moved"), c.uninstallTarget.Name, failureDetail(failures))
		})
	}()
}

func shortenUninstallError(message string) string {
	runes := []rune(message)
	if len(runes) > 120 {
		return string(runes[:120])
	}
	return message
}

func failureDetail(failures map[string]string) string {
	paths := make([]string, 0, len(failures))
	for path := range failures {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	lines := make([]string, 0, len(paths))
	for _, path := range paths {
		lines = append(lines, path+" — "+failures[path])
	}
	return strings.Join(lines, "\n")
}

// removeLauncherReferences clears only the launcher metadata of the app whose
// install folder actually left the machine.
func (c *controller) removeLauncherReferences(application domain.Application) {
	if c.launcherPrefs == nil {
		return
	}
	id := application.ID
	if id == "" {
		return
	}
	if c.launcherPrefs.IsFavorite(id) {
		if err := c.launcherPrefs.ToggleFavorite(id); err != nil {
			log.Printf("uninstall favorite cleanup failed: %v", err)
		}
	}
	if err := c.launcherPrefs.SetHidden(id, false); err != nil {
		log.Printf("uninstall visibility cleanup failed: %v", err)
	}
	if err := c.launcherPrefs.SetAlias(id, ""); err != nil {
		log.Printf("uninstall alias cleanup failed: %v", err)
	}
	c.launcherPrefs.ResetRanking(id)
	c.queueRankingSave()
	if c.launcher != nil {
		c.launcher.SetApps(c.catalog)
	}
	c.refreshExtras()
	c.scan()
}

func (c *controller) copyUninstallPath(path string) {
	if path == "" {
		return
	}
	c.copyText(path, false)
}

func (c *controller) revealUninstallPath(path string) {
	if path == "" {
		return
	}
	c.hide()
	mygo.Shell.ShowItemInFolder(path)
}

func (c *controller) uninstallMenu() {
	selected, ok := c.selectedUninstall()
	selectedCount := 0
	if c.view != nil {
		selectedCount = domainuninstall.SelectedCount(c.view.UninstallAll)
	}
	menu := components.Menu([]components.MenuAction{
		{Label: c.text("卸载选中项…", "Uninstall selected…"), Disabled: selectedCount == 0, Click: c.confirmUninstall},
		{Label: c.text("选择/取消选择", "Select / unselect"), Accelerator: "Ctrl+Enter",
			Disabled: !ok || !selected.Removable, Click: func() { c.view.ToggleUninstall(c.view.Chosen) }},
		{Label: c.text("复制路径", "Copy path"), Accelerator: "Ctrl+Shift+C", Disabled: !ok, Click: func() {
			if ok {
				c.copyUninstallPath(selected.Path)
			}
		}},
		{Label: c.text("在文件夹中显示", "Show in folder"), Disabled: !ok, Click: func() {
			if ok {
				c.revealUninstallPath(selected.Path)
			}
		}},
		{Separator: true},
		{Label: c.text("重新扫描", "Scan again"), Click: c.scanUninstall},
		{Label: c.text("返回启动器", "Back to launcher"), Accelerator: "Esc", Click: c.view.SetModeLauncher},
		{Separator: true},
		{Label: c.text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.text("退出", "Quit"), Click: mygo.App.Quit},
	})
	menu.Popup(c.panel)
}
