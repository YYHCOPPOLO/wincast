package app

import (
	"log"
	"os"
	"path/filepath"
	"strconv"

	"github.com/YYHCOPPOLO/wincast/go/internal/features/settings"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

const onboardingDoneFile = "onboarding-done"

func (c *controller) onboardingDonePath() string {
	return filepath.Join(c.profile.settings, onboardingDoneFile)
}

// showOnboardingIfNeeded opens the first-run window once per profile. The
// legacy app used the same marker file, so an upgraded profile does not see it
// again.
func (c *controller) showOnboardingIfNeeded() {
	if _, err := os.Stat(c.onboardingDonePath()); err == nil {
		return
	}
	if legacy, err := os.UserConfigDir(); err == nil {
		if _, err := os.Stat(filepath.Join(legacy, storage.DailyID, onboardingDoneFile)); err == nil {
			c.finishOnboarding()
			return
		}
	}
	c.showOnboarding()
}

func (c *controller) showOnboarding() {
	if c.onboarding != nil {
		showWindow(c.onboarding)
		return
	}
	c.onboardingStep = 0
	c.onboarding = mygo.NewWindow(mygo.WindowOptions{
		Title: c.text("欢迎使用 Tinycast", "Welcome to Tinycast"),
		Width: 520, Height: 400, Hidden: true, DisableResize: true,
		BackgroundColor: "light-dark(#f4f4f6, #1a1a1e)",
		Content:         ui.View(c.onboardingView),
	})
	c.onboarding.OnClosed(func() {
		c.finishOnboarding()
		c.onboarding = nil
	})
	c.onboarding.Center()
	showWindow(c.onboarding)
}

func (c *controller) onboardingView(ctx *ui.Context) {
	tokens := theme.For(c.dark())
	ctx.SetTheme(tokens.Theme())
	ctx.Root().Background(tokens.Page)
	zh := c.language == "zh"
	ui.Column(ctx).Fill().Padding(36).Gap(14).Justify(ui.Center).Children(func() {
		ui.Text(ctx, onboardingTitle(c.onboardingStep, zh)).FontSize(24).Bold().Role(ui.RoleHeading)
		ui.Text(ctx, onboardingSubtitle(c.onboardingStep, zh)).FontSize(theme.SectionFont).
			TextColor(tokens.TextSecondary)
		switch c.onboardingStep {
		case 0:
			ui.TextInput(ctx, &c.hotkeyDraft).Label(c.text("全局快捷键", "Global shortcut")).
				Placeholder(previewShortcut).Grow(1)
			ui.Text(ctx, c.text("例如 Alt+Shift+Space；点击“应用”立即生效。", "For example Alt+Shift+Space; Apply takes effect immediately.")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
			if ui.Button(ctx, c.text("应用快捷键", "Apply shortcut")).Clicked() {
				c.setHotkey(c.hotkeyDraft)
			}
		case 1:
			ui.Text(ctx, c.text("Tinycast 使用 UI Automation 把内容粘贴回前台应用。Windows 不会在启动时弹出权限提示。",
				"Tinycast uses UI Automation to paste back into the frontmost app. Windows does not prompt at launch.")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
			if ui.Button(ctx, c.text("打开权限设置", "Open permission settings")).Clicked() {
				c.openPermissionSettings("ms-settings:privacy-general")
			}
		case 2:
			ui.Text(ctx, c.text("可在设置 → 备份 → 从 Raycast 导入中带上设置与片段；密钥与自动能力不会随导入开启。",
				"Settings → Backup → Import from Raycast can carry settings and snippets; keys and capability grants are never enabled by an import.")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
			if ui.Button(ctx, c.text("打开备份设置", "Open backup settings")).Clicked() {
				c.settingsTab = settings.Backup
				if c.onboarding != nil {
					c.onboarding.Hide()
				}
				c.openSettings()
			}
		}
		ui.Row(ctx).Grow(1).Justify(ui.End).AlignItems(ui.End).Children(func() {
			label := c.text("继续", "Continue")
			if c.onboardingStep == 3 {
				label = c.text("开始使用", "Get Started")
			}
			if ui.PrimaryButton(ctx, label).Clicked() {
				if c.onboardingStep >= 3 {
					c.finishOnboarding()
					if c.onboarding != nil {
						c.onboarding.Hide()
					}
					return
				}
				c.onboardingStep++
			}
		})
		ui.Text(ctx, c.text("第 "+strconv.Itoa(c.onboardingStep+1)+" / 4 步", "Step "+strconv.Itoa(c.onboardingStep+1)+" of 4")).
			FontSize(theme.KeyCapFont).TextColor(tokens.TextTertiary)
	})
}

func (c *controller) finishOnboarding() {
	if err := storage.WriteFileAtomic(c.onboardingDonePath(), []byte("1\n"), 0o600); err != nil {
		log.Printf("onboarding marker failed: %v", err)
	}
}

func onboardingTitle(step int, zh bool) string {
	if zh {
		switch step {
		case 0:
			return "欢迎使用 Tinycast"
		case 1:
			return "允许粘贴"
		case 2:
			return "从 Raycast 导入"
		default:
			return "一切就绪"
		}
	}
	switch step {
	case 0:
		return "Welcome to Tinycast"
	case 1:
		return "Enable Pasting"
	case 2:
		return "Import from Raycast"
	default:
		return "You're all set"
	}
}

func onboardingSubtitle(step int, zh bool) string {
	if zh {
		switch step {
		case 0:
			return "设置快捷键，从任意位置呼出启动器。"
		case 1:
			return "允许 Tinycast 把内容粘贴回你刚才使用的应用。"
		case 2:
			return "带上你的快捷键、收藏和剪贴板历史。"
		default:
			return "Tinycast 已就绪。随时按快捷键开始。"
		}
	}
	switch step {
	case 0:
		return "Set a shortcut to summon the launcher from anywhere."
	case 1:
		return "Let Tinycast paste items back into the app you were using."
	case 2:
		return "Bring your shortcuts, favorites and clipboard history along."
	default:
		return "Tinycast is ready. Press your shortcut anytime to start."
	}
}
