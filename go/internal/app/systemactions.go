package app

import (
	"errors"
	"fmt"
	"log"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/systemaction"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/egoist/mygo"
)

func (c *controller) systemActionMenu() []components.MenuAction {
	chinese := c.language == "zh"
	action := func(id systemaction.ID) components.MenuAction {
		return components.MenuAction{Label: id.Name(chinese), Click: func() { c.runSystemAction(id) }}
	}
	return []components.MenuAction{
		action(systemaction.LockScreen),
		action(systemaction.Sleep),
		action(systemaction.SleepDisplays),
		action(systemaction.ShowScreenSaver),
		{Separator: true},
		action(systemaction.Restart),
		action(systemaction.ShutDown),
		action(systemaction.LogOut),
		{Separator: true},
		action(systemaction.PlayPause),
		action(systemaction.NextTrack),
		action(systemaction.PreviousTrack),
		action(systemaction.ToggleMute),
		action(systemaction.VolumeUp),
		action(systemaction.VolumeDown),
		action(systemaction.SetVolume),
		action(systemaction.Volume0),
		action(systemaction.Volume25),
		action(systemaction.Volume50),
		action(systemaction.Volume75),
		action(systemaction.Volume100),
		{Separator: true},
		action(systemaction.ShowDesktop),
		action(systemaction.ToggleAppearance),
		action(systemaction.ToggleStageManager),
		action(systemaction.OpenTrash),
		action(systemaction.EmptyTrash),
		action(systemaction.EjectAllDisks),
		action(systemaction.ToggleHiddenFiles),
		{Separator: true},
		action(systemaction.HideOtherApps),
		action(systemaction.UnhideAllApps),
		action(systemaction.QuitAllApps),
		action(systemaction.DismissNotifications),
		action(systemaction.ToggleBluetooth),
	}
}

// runSystemAction is the one gate for confirmations; the platform runner is
// only called after the user accepted.
func (c *controller) runSystemAction(id systemaction.ID) {
	if c.closing {
		return
	}
	if id == systemaction.SetVolume {
		c.openVolumeDialog()
		return
	}
	confirmation := id.Confirm(c.language == "zh")
	if !confirmation.Required {
		c.executeSystemAction(id)
		return
	}
	if c.systemConfirming != "" {
		return
	}
	title := confirmation.Title
	message := confirmation.Message
	accept := c.text("继续", "Continue")
	if confirmation.Computed {
		targets := windows.QuitAllTargets()
		title = c.text("退出所有应用？", "Quit all applications?")
		message = fmt.Sprintf(c.text("%d 个应用将收到关闭请求。", "%d applications will be asked to close."), len(targets))
		accept = c.text("退出", "Quit")
	}
	c.systemConfirming = id
	c.hide()
	options := mygo.MessageOptions{
		Parent: c.panel, Type: mygo.MessageQuestion,
		Title: title, Message: message,
		Buttons:       []string{c.text("取消", "Cancel"), accept},
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
			c.systemConfirming = ""
			if c.closing {
				return
			}
			if err != nil {
				log.Printf("system action confirmation failed: %v", err)
				return
			}
			if result.Button == 1 {
				c.executeSystemAction(id)
			}
		})
	}()
}

func (c *controller) executeSystemAction(id systemaction.ID) {
	if c.closing {
		return
	}
	c.hide()
	log.Printf("system action start id=%s", id)
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		result, err := windows.ExecuteSystemAction(string(id))
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing {
				return
			}
			if err != nil {
				log.Printf("system action failed id=%s error=%v", id, err)
				c.reportSystemActionFailure(id, err)
				return
			}
			message := c.localizedActionMessage(result.Message)
			if message == "" && id.ShowsVolume() {
				if level, muted, readErr := windows.CurrentVolume(); readErr == nil {
					message = fmt.Sprintf(c.text("音量 %d%%", "Volume %d%%"), int(level*100+0.5))
					if muted {
						message = c.text("已静音", "Muted")
					}
				}
			}
			if message != "" {
				c.view.PendingToast = message
			}
			log.Printf("system action done id=%s message=%q", id, result.Message)
		})
	}()
}

func (c *controller) reportSystemActionFailure(id systemaction.ID, err error) {
	message := err.Error()
	var actionErr *windows.SystemActionError
	if errors.As(err, &actionErr) {
		message = actionErr.Message
	}
	c.view.PendingToast = message
	c.showFailure(fmt.Sprintf(c.text("“%s”失败", "“%s” failed"), id.Name(c.language == "zh")), id.Name(c.language == "zh"), message)
	if actionErr != nil && actionErr.SettingsURI != "" {
		if openErr := mygo.Shell.OpenExternal(actionErr.SettingsURI); openErr != nil {
			log.Printf("open settings uri failed: %v", openErr)
		}
	}
}

func (c *controller) localizedActionMessage(message string) string {
	if message == "" {
		return ""
	}
	if c.language != "zh" {
		return message
	}
	switch {
	case message == "Trash Is Already Empty":
		return "回收站已为空"
	case message == "Trash Emptied":
		return "回收站已清空"
	case message == "Hidden Files Shown":
		return "已显示隐藏文件"
	case message == "Hidden Files Hidden":
		return "已隐藏文件"
	case message == "No Disks to Eject":
		return "没有可弹出的磁盘"
	case message == "1 Disk Ejected":
		return "已弹出 1 个磁盘"
	case message == "Nothing Was Hidden":
		return "没有需要取消隐藏的窗口"
	case message == "All Apps Unhidden":
		return "已取消隐藏所有应用"
	case message == "Dark Appearance":
		return "深色外观"
	case message == "Light Appearance":
		return "浅色外观"
	case message == "Bluetooth On":
		return "蓝牙已开启"
	case message == "Bluetooth Off":
		return "蓝牙已关闭"
	case strings.HasPrefix(message, "Stage Manager"):
		return "Windows 不支持台前调度"
	case strings.HasPrefix(message, "Dismissing notifications"):
		return "Windows 不支持清除通知"
	case strings.HasSuffix(message, " Disks Ejected"):
		return strings.TrimSuffix(message, " Disks Ejected") + " 个磁盘已弹出"
	}
	return message
}

func (c *controller) openVolumeDialog() {
	if c.closing || c.volumeLoading {
		return
	}
	c.volumeLoading = true
	if !c.view.VolumeOpen {
		c.view.VolumeValue = 0.5
	}
	c.view.VolumeOpen = true
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
		c.showPanel(false)
	} else {
		c.panel.Update(func() {})
	}
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		level, _, err := windows.CurrentVolume()
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			c.volumeLoading = false
			if c.closing {
				return
			}
			if err == nil {
				c.view.VolumeValue = level
			}
		})
	}()
}

func (c *controller) applyVolume(value float64) {
	if c.closing {
		return
	}
	c.hide()
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		err := windows.SetVolume(value)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing {
				return
			}
			if err != nil {
				c.reportSystemActionFailure(systemaction.SetVolume, err)
				return
			}
			c.view.PendingToast = fmt.Sprintf(c.text("音量 %d%%", "Volume %d%%"), int(value*100+0.5))
		})
	}()
}

func (c *controller) closeVolumeDialog() {}
