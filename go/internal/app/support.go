package app

import (
	"encoding/json"
	"log"
	"os"
	"path/filepath"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

const supportInterval = 30 * 24 * time.Hour

type supportState struct {
	FirstSeenAt int64 `json:"firstSeenAt"`
	LastAskedAt int64 `json:"lastAskedAt"`
}

func (c *controller) supportStatePath() string {
	return filepath.Join(c.profile.settings, "support-reminder.json")
}

// loadSupportState keeps the anchor in a file, not in settings: a settings
// import must not reset when the next ask is due.
func (c *controller) loadSupportState() supportState {
	var state supportState
	if raw, err := os.ReadFile(c.supportStatePath()); err == nil {
		if json.Unmarshal(raw, &state) == nil && state.FirstSeenAt > 0 {
			return state
		}
	}
	state.FirstSeenAt = time.Now().Unix()
	if err := c.saveSupportState(state); err != nil {
		log.Printf("support state save failed: %v", err)
	}
	return state
}

func (c *controller) saveSupportState(state supportState) error {
	raw, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return err
	}
	return storage.WriteFileAtomic(c.supportStatePath(), append(raw, '\n'), 0o600)
}

// supportDue reports whether the 30-day interval has elapsed since the last
// ask, or since first run when the window was never shown.
func supportDue(firstSeenAt, lastAskedAt int64, now time.Time) bool {
	anchor := firstSeenAt
	if lastAskedAt > anchor {
		anchor = lastAskedAt
	}
	if anchor <= 0 {
		return false
	}
	elapsed := now.Unix() - anchor
	if elapsed < 0 {
		elapsed = 0
	}
	if elapsed > int64(supportInterval/time.Second) {
		elapsed = int64(supportInterval / time.Second)
	}
	return elapsed >= int64(supportInterval/time.Second)
}

func (c *controller) startSupportReminders() {
	if !c.supportReminders || c.supportLoaded {
		return
	}
	state := c.loadSupportState()
	c.supportFirstSeenAt, c.supportAskedAt, c.supportLoaded = state.FirstSeenAt, state.LastAskedAt, true
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		ticker := time.NewTicker(30 * time.Minute)
		defer ticker.Stop()
		c.maybeShowSupport()
		for {
			select {
			case <-c.ctx.Done():
				return
			case <-ticker.C:
				c.maybeShowSupport()
			}
		}
	}()
}

func (c *controller) maybeShowSupport() {
	if c.panel == nil {
		return
	}
	c.panel.Update(func() {
		if c.closing || !c.supportReminders || !c.supportLoaded {
			return
		}
		if c.supportWindow != nil || c.onboarding != nil {
			return
		}
		if c.panel.IsVisible() || (c.settings != nil && c.settings.IsVisible()) {
			return
		}
		if !supportDue(c.supportFirstSeenAt, c.supportAskedAt, time.Now()) {
			return
		}
		c.showSupportWindow()
	})
}

func (c *controller) showSupportWindow() {
	if c.supportWindow != nil {
		showWindow(c.supportWindow)
		return
	}
	c.supportWindow = c.newMaterialWindow(mygo.WindowOptions{
		Title: c.text("支持 Tinycast", "Support Tinycast"),
		Width: 460, Height: 320, Hidden: true, DisableResize: true,
		BackgroundColor: "light-dark(#f4f4f6, #1a1a1e)",
		Content:         ui.View(c.supportView),
	})
	c.supportWindow.OnClosed(func() { c.supportWindow = nil })
	c.supportWindow.Center()
	c.markSupportAsked()
	showWindow(c.supportWindow)
}

func (c *controller) markSupportAsked() {
	c.supportAskedAt = time.Now().Unix()
	if err := c.saveSupportState(supportState{FirstSeenAt: c.supportFirstSeenAt, LastAskedAt: c.supportAskedAt}); err != nil {
		log.Printf("support state save failed: %v", err)
	}
}

func (c *controller) supportView(ctx *ui.Context) {
	tokens := theme.ForContext(ctx)
	ctx.SetTheme(tokens.Theme())
	defer ctx.Root().Background(tokens.PageBackground(ctx.Vibrancy()))
	ui.Column(ctx).Fill().Padding(32).Gap(14).Justify(ui.Center).Children(func() {
		ui.Text(ctx, c.text("支持 Tinycast", "Support Tinycast")).FontSize(24).Bold().Role(ui.RoleHeading)
		ui.Text(ctx, c.text("一个小启动器。你的支持让它保持独立。", "A small launcher. Support keeps it independent.")).
			FontSize(theme.SectionFont).TextColor(tokens.TextSecondary)
		if ui.PrimaryButton(ctx, c.text("在 Polar 支持", "Support on Polar")).Clicked() {
			if err := openExternalURL(supportCheckoutURL); err != nil {
				log.Printf("support checkout failed: %v", err)
			}
		}
		components.FormRow(ctx, tokens, c.text("支持提醒", "Support reminders"), func() {
			enabled := c.supportReminders
			ui.Switch(ctx, &enabled).Label(c.text("允许偶尔再次询问", "Allow an occasional reminder"))
			if enabled != c.supportReminders {
				c.setSupportReminders(enabled)
			}
		})
	})
}
