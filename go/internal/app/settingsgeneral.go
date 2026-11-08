package app

import (
	"fmt"
	"log"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/quicklinks"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/settings"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

// setGeneralBool persists one boolean setting and runs its side effect first.
// Persistence happens only after the side effect succeeds, so a failed tray or
// startup write never claims to have taken effect.
func (c *controller) setGeneralBool(key string, value bool, apply func()) {
	if apply != nil {
		apply()
	}
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set(key, value)
	}
	c.queueSettingsSave()
	c.refreshSettingsWindow()
}

func (c *controller) setOpenOnCursorScreen(enabled bool) {
	c.openOnCursorScreen = enabled
	c.setGeneralBool("openOnCursorScreen", enabled, nil)
}

func (c *controller) setLaunchAtLogin(enabled bool) {
	if err := windows.ApplyLaunchAtLogin(settings.LaunchAtLoginKey, enabled); err != nil {
		c.settingsStatus = c.text("无法写入 Windows 开机启动项", "Could not update the Windows startup entry")
		log.Printf("launch at login failed: %v", err)
		return
	}
	c.launchAtLogin = enabled
	c.setGeneralBool("launchAtLogin", enabled, nil)
	c.settingsStatus = c.text("开机启动已更新", "Startup entry updated")
}

func (c *controller) setShowInTray(enabled bool) {
	if enabled && c.tray == nil {
		if err := c.setupTray(); err != nil {
			c.settingsStatus = c.text("无法创建托盘图标", "Could not create the tray icon")
			log.Printf("tray enable failed: %v", err)
			return
		}
	}
	if !enabled && c.tray != nil {
		c.tray.Destroy()
		c.tray = nil
	}
	c.showInTray = enabled
	c.setGeneralBool("showInMenuBar", enabled, nil)
	c.settingsStatus = c.text("托盘图标已更新", "Tray icon updated")
}

func (c *controller) cyclePopToRoot() {
	presets := []int64{0, 5, 15, 30, 60, 90}
	next := int64(0)
	for index, preset := range presets {
		if preset == c.popToRootTimeout {
			next = presets[(index+1)%len(presets)]
			break
		}
	}
	c.popToRootTimeout = next
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("popToRootTimeout", next)
	}
	c.queueSettingsSave()
	c.refreshSettingsWindow()
}

func (c *controller) popToRootLabel() string {
	switch c.popToRootTimeout {
	case 0:
		return c.text("立即", "Immediately")
	case 5, 15, 30, 60, 90:
		return c.text(fmt.Sprintf("关闭后 %d 秒", c.popToRootTimeout), fmt.Sprintf("After %d s", c.popToRootTimeout))
	}
	return c.text("立即", "Immediately")
}

func (c *controller) setShowFavoritesInCompact(enabled bool) {
	c.showFavoritesInCompact = enabled
	c.refreshCompactFavorites()
	c.setGeneralBool("showFavoritesInCompactMode", enabled, nil)
	if c.panel != nil {
		c.panel.Update(func() {})
	}
}

func (c *controller) setPaletteDraggable(enabled bool) {
	c.paletteDraggable = enabled
	c.view.Draggable = enabled
	c.setGeneralBool("paletteDraggable", enabled, nil)
	if c.panel != nil {
		c.panel.Update(func() {})
	}
}

func (c *controller) setSupportReminders(enabled bool) {
	c.supportReminders = enabled
	c.setGeneralBool("supportReminders", enabled, nil)
}

func (c *controller) setExtensionsEnabled(enabled bool) {
	c.extensionsEnabled = enabled
	c.setGeneralBool("extensionsEnabled", enabled, nil)
}

func (c *controller) setExtensionsShowInLauncher(enabled bool) {
	c.extensionsShowInLauncher = enabled
	c.setGeneralBool("extensionsShowInLauncher", enabled, nil)
}

func (c *controller) setApplicationsVisible(enabled bool) {
	c.applicationsShowInLauncher = enabled
	if c.launcher != nil {
		c.launcher.SetApplicationsEnabled(enabled)
	}
	c.setGeneralBool("applicationsShowInLauncher", enabled, nil)
	c.view.Filter()
}

func (c *controller) setSystemSettingsVisible(enabled bool) {
	c.systemSettingsShowInLauncher = enabled
	c.setGeneralBool("systemSettingsShowInLauncher", enabled, nil)
	c.refreshExtras()
	c.view.Filter()
}

func (c *controller) setSystemActionsVisible(enabled bool) {
	c.systemActionsShowInLauncher = enabled
	c.setGeneralBool("systemActionsShowInLauncher", enabled, nil)
	c.refreshExtras()
	c.view.Filter()
}

func (c *controller) resetLearnedRanking() {
	if c.launcherPrefs == nil {
		return
	}
	if err := c.launcherPrefs.ClearRanking(); err != nil {
		c.settingsStatus = c.text("无法重置学习排序", "Could not reset learned ranking")
		log.Printf("reset ranking failed: %v", err)
		return
	}
	c.settingsStatus = c.text("学习排序已重置", "Learned ranking reset")
	c.view.Filter()
	c.refreshSettingsWindow()
}

func (c *controller) saveSettingsAlias(id, alias string) {
	if err := c.launcherPrefs.SetAlias(id, alias); err != nil {
		c.settingsStatus = c.text("别名保存失败", "Could not save the alias")
		log.Printf("settings alias failed: %v", err)
		return
	}
	c.launcher.SetApps(c.catalog)
	c.refreshExtras()
	c.view.Filter()
	c.settingsAliasID = ""
	c.settingsAliasDraft = ""
	c.settingsStatus = c.text("别名已保存", "Alias saved")
	c.refreshSettingsWindow()
}

func (c *controller) toggleEntryFavorite(application domain.Application) {
	if c.launcherPrefs == nil {
		return
	}
	if err := c.launcherPrefs.ToggleFavorite(application.ID); err != nil {
		c.settingsStatus = c.text("收藏保存失败", "Could not save the favorite")
		log.Printf("settings favorite failed: %v", err)
		return
	}
	if c.launcherPrefs.IsFavorite(application.ID) {
		c.settingsStatus = c.text("已收藏", "Added to favorites")
	} else {
		c.settingsStatus = c.text("已取消收藏", "Removed from favorites")
	}
	c.refreshCompactFavorites()
	c.view.Filter()
	c.refreshSettingsWindow()
}

func (c *controller) toggleEntryHidden(application domain.Application) {
	hidden := !c.launcherPrefs.IsHidden(application.ID)
	if err := c.launcherPrefs.SetHidden(application.ID, hidden); err != nil {
		c.settingsStatus = c.text("隐藏状态保存失败", "Could not save visibility")
		log.Printf("settings visibility failed: %v", err)
		return
	}
	if hidden {
		c.settingsStatus = c.text("已隐藏", "Hidden")
	} else {
		c.settingsStatus = c.text("已显示", "Visible")
	}
	c.view.Filter()
	c.refreshSettingsWindow()
}

func (c *controller) refreshCompactFavorites() {
	if c.view == nil || c.launcherPrefs == nil {
		return
	}
	c.view.ShowFavoritesInCompact = c.showFavoritesInCompact
	favorites := make([]domain.Application, 0, 5)
	for _, id := range c.launcherPrefs.FavoriteOrder() {
		if application, ok := c.launcher.Application(id); ok {
			favorites = append(favorites, application)
			if len(favorites) == 5 {
				break
			}
		}
	}
	c.view.CompactFavorites = favorites
}

func (c *controller) openPermissionSettings(uri string) {
	if err := openExternalURL(uri); err != nil {
		c.settingsStatus = c.text("无法打开 Windows 设置", "Could not open Windows Settings")
		log.Printf("open permission settings failed: %v", err)
		return
	}
	c.settingsStatus = c.text("已打开 Windows 设置", "Windows Settings opened")
}

func openExternalURL(uri string) error {
	return quicklinks.Open(uri)
}
