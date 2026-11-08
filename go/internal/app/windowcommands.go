package app

import (
	"log"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowcommand"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/windowmanagement"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
)

func (c *controller) windowCommandMenu() []components.MenuAction {
	chinese := c.language == "zh"
	action := func(id windowcommand.ID) components.MenuAction {
		return components.MenuAction{Label: id.Name(chinese), Click: func() { c.runWindowCommand(id) }}
	}
	groups := []windowcommand.Group{
		windowcommand.GroupHalves,
		windowcommand.GroupQuarters,
		windowcommand.GroupFourths,
		windowcommand.GroupThirds,
		windowcommand.GroupSizing,
		windowcommand.GroupMoving,
		windowcommand.GroupFullscreen,
		windowcommand.GroupSpaces,
	}
	items := make([]components.MenuAction, 0, len(windowcommand.All)+len(groups)*2)
	for _, group := range groups {
		if len(items) > 0 {
			items = append(items, components.MenuAction{Separator: true})
		}
		items = append(items, components.MenuAction{Label: windowcommand.GroupTitle(group, chinese), Disabled: true})
		for _, id := range windowcommand.All {
			if id.Group() == group {
				items = append(items, action(id))
			}
		}
	}
	return items
}

// actionTarget keeps the window that was foreground before the palette opened.
// A hotkey with the palette hidden acts on the current foreground window.
func (c *controller) actionTarget() uintptr {
	if c.panel == nil {
		return c.target.Window
	}
	target := windowmanagement.WindowTarget(c.target.Window, c.panel.IsVisible(), c.ownsForeground())
	if target == 0 {
		target = c.target.Window
	}
	return target
}

func (c *controller) runWindowCommand(id windowcommand.ID) {
	if c.closing {
		return
	}
	if !c.featureWindowManagement {
		c.view.PendingToast = c.text("窗口管理已关闭", "Window management is disabled")
		return
	}
	target := c.actionTarget()
	if target == 0 {
		c.view.PendingToast = c.text("没有可操作的前台窗口", "There is no foreground window to act on")
		return
	}
	c.hide()
	log.Printf("window command start id=%s %s", id, describeWindowTarget(target))
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		moved, message := c.windowManager.Perform(target, id, float64(c.windowGap), c.windowCycle)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing {
				return
			}
			if message != "" {
				c.view.PendingToast = c.localizedWindowMessage(message)
				return
			}
			if !moved {
				c.view.PendingToast = c.text("窗口无法调整", "The window could not be adjusted")
				return
			}
			log.Printf("window command done id=%s", id)
		})
	}()
}

func (c *controller) localizedWindowMessage(message string) string {
	if c.language != "zh" {
		return message
	}
	switch message {
	case "Switching spaces is not available on Windows.":
		return "Windows 不支持切换虚拟桌面"
	}
	return message
}

func describeWindowTarget(hwnd uintptr) string {
	return windows.DescribeWindowTarget(hwnd)
}
