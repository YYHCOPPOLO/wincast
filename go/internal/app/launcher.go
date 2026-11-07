package app

import (
	"encoding/json"
	"log"
	"path/filepath"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/launcher"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
)

func (c *controller) search(query string, showHidden bool) []domain.Application {
	if c.launcher == nil {
		return nil
	}
	return c.launcher.Search(query, showHidden, time.Now())
}

func (c *controller) selected() (domain.Application, bool) {
	if c.view == nil || c.view.Mode != palette.ModeLauncher || c.view.Chosen < 0 || c.view.Chosen >= len(c.view.Results) {
		return domain.Application{}, false
	}
	return c.view.Results[c.view.Chosen], true
}

func (c *controller) openFavorite(digit byte) {
	if c.launcherPrefs == nil {
		return
	}
	id := c.launcherPrefs.FavoriteSlot(digit)
	if id == "" {
		c.view.Status = c.view.Text("该收藏槽位为空", "That favorite slot is empty")
		return
	}
	application, ok := c.launcher.Application(id)
	if !ok {
		c.view.Status = c.view.Text("收藏的应用已不存在", "The favorite application is gone")
		c.panel.Update(func() {})
		return
	}
	c.open(application)
}

func (c *controller) toggleFavorite() {
	application, ok := c.selected()
	if !ok {
		return
	}
	if err := c.launcherPrefs.ToggleFavorite(application.ID); err != nil {
		c.view.Status = c.view.Text("收藏保存失败", "Could not save the favorite")
		log.Printf("favorite save failed: %v", err)
	} else if c.launcherPrefs.IsFavorite(application.ID) {
		c.view.Status = c.view.Text("已收藏", "Added to favorites")
	} else {
		c.view.Status = c.view.Text("已取消收藏", "Removed from favorites")
	}
	c.view.Filter()
}

func (c *controller) saveAlias(application domain.Application, alias string) {
	if err := c.launcherPrefs.SetAlias(application.ID, alias); err != nil {
		c.view.Status = c.view.Text("别名保存失败", "Could not save the alias")
		log.Printf("alias save failed: %v", err)
		return
	}
	c.launcher.SetApps(c.catalog)
	c.view.Status = c.view.Text("别名已保存", "Alias saved")
	c.view.Filter()
}

func (c *controller) setHidden(application domain.Application, hidden bool) {
	if err := c.launcherPrefs.SetHidden(application.ID, hidden); err != nil {
		c.view.Status = c.view.Text("隐藏状态保存失败", "Could not save visibility")
		log.Printf("hidden save failed: %v", err)
		return
	}
	c.view.Status = c.view.Text("已隐藏此应用", "Application hidden")
	if !hidden {
		c.view.Status = c.view.Text("已显示此应用", "Application shown")
	}
	c.view.Filter()
}

func (c *controller) toggleShowHidden() {
	c.view.ShowHidden = !c.view.ShowHidden
	c.view.Filter()
}

func (c *controller) openAliasDialog() {
	application, ok := c.selected()
	if !ok {
		return
	}
	c.view.AliasTarget = application
	c.view.AliasText = c.launcherPrefs.Alias(application.ID)
	c.view.AliasOpen = true
	c.panel.Update(func() {})
	// The first update builds the modal; the next frame can focus its input.
	c.panel.Update(func() { c.view.FocusAlias() })
}

func (c *controller) toggleExpanded() {
	c.view.SetExpanded(!c.view.Expanded)
	c.compactMode = !c.view.Expanded
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("compactMode", c.compactMode)
	}
	c.queueSettingsSave()
	c.panel.Update(func() {})
}

func (c *controller) panelHeight() int {
	if c.view != nil && !c.view.Expanded {
		return int(theme.CompactHeight)
	}
	return int(theme.PanelHeight)
}

func (c *controller) applyPanelSize() {
	if c.panel == nil {
		return
	}
	width, height := c.panel.Size()
	if width == int(theme.PanelWidth) && height == c.panelHeight() {
		return
	}
	x, y := c.panel.Position()
	c.panel.SetSize(int(theme.PanelWidth), c.panelHeight())
	c.panel.SetPosition(x, y)
}

// Query changes may originate while a view is being built. Defer native
// geometry mutations to one queued update and retain the existing top anchor.
func (c *controller) schedulePanelSize() {
	if c.panel == nil || c.panelSizePending || c.closing {
		return
	}
	c.panelSizePending = true
	c.panel.Update(func() {
		c.panelSizePending = false
		if !c.closing {
			c.applyPanelSize()
		}
	})
}

func (c *controller) customDirectories() []string {
	document, _ := c.settingDocument()
	if document == nil {
		return nil
	}
	raw, ok := document.Raw("customAppDirs")
	if !ok {
		return nil
	}
	var directories []string
	if err := json.Unmarshal(raw, &directories); err != nil {
		log.Printf("custom app directories ignored: %v", err)
		return nil
	}
	return directories
}

func (c *controller) queueRankingSave() {
	if c.launcherPrefs == nil {
		return
	}
	data, err := c.launcherPrefs.RankingBytes()
	if err != nil {
		log.Printf("ranking encode failed: %v", err)
		return
	}
	select {
	case <-c.rankingWrites:
	default:
	}
	select {
	case c.rankingWrites <- data:
	default:
	}
}

func (c *controller) startRankingWriter() {
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		persist := func(data []byte) {
			path := filepath.Join(c.profile.settings, "launcher", "ranking.json")
			if err := storage.WriteFileAtomic(path, data, 0o600); err != nil {
				log.Printf("ranking save failed: %v", err)
			}
		}
		for {
			select {
			case <-c.ctx.Done():
				if data, ok := newestPending(c.rankingWrites); ok {
					persist(data)
				}
				return
			case data := <-c.rankingWrites:
				persist(data)
			}
		}
	}()
}

func (c *controller) scanCustomApplications(directories []string) []domain.Application {
	if len(directories) == 0 {
		return nil
	}
	apps, err := launcher.ScanCustomApps(c.ctx, directories)
	if err != nil {
		log.Printf("custom application scan failed: %v", err)
	}
	return apps
}
