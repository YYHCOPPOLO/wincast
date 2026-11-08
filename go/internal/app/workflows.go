package app

import (
	"encoding/json"
	"errors"
	"log"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/quicklink"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/snippet"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/commands"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/snippets"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/editors"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

// workflowFeature renders one feature switch in a settings card.
func (c *controller) workflowFeature(ctx *ui.Context, tokens theme.Tokens, label string, enabled bool, set func(bool)) {
	components.FormRow(ctx, tokens, label, func() {
		value := enabled
		ui.Switch(ctx, &value).Label(label)
		if value != enabled {
			set(value)
		}
	})
}

func (c *controller) launcherVisibility(ctx *ui.Context, tokens theme.Tokens, key string, value bool) {
	components.FormRow(ctx, tokens, c.text("启动器显示", "Show in launcher"), func() {
		enabled := value
		ui.Switch(ctx, &enabled)
		if enabled != value {
			c.setFeature(key, enabled)
		}
	})
}

func (c *controller) workflowRow(ctx *ui.Context, tokens theme.Tokens, name, subtitle, hotkey, extraLabel string, extra, edit, remove func()) {
	ui.Row(ctx).FillWidth().Padding(4, 8).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Column(ctx).Grow(1).Gap(1).Children(func() {
			ui.Text(ctx, name).FontSize(theme.RowFont).TextColor(tokens.Text).SingleLine()
			line := subtitle
			if hotkey != "" {
				line += " · " + hotkey
			}
			ui.Text(ctx, line).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).SingleLine()
		})
		if extra != nil && extraLabel != "" {
			if ui.Button(ctx, extraLabel).Clicked() {
				extra()
			}
		}
		if ui.Button(ctx, c.text("编辑", "Edit")).Clicked() {
			edit()
		}
		if ui.Button(ctx, c.text("删除", "Delete")).Clicked() {
			remove()
		}
	})
}

func (c *controller) toggleQuicklinkPin(link quicklink.Link) {
	if c.quicklinks == nil {
		return
	}
	if _, err := c.quicklinks.TogglePin(link.ID); err != nil {
		c.settingsStatus = c.text("置顶保存失败", "Could not save the pin")
		log.Printf("quicklink pin failed: %v", err)
	} else {
		c.settingsStatus = c.text("已更新置顶", "Pin updated")
	}
	c.refreshExtras()
	c.settings.Update(func() {})
}

func (c *controller) newCommand() {
	c.formModel = &editors.Model{Open: true, Kind: editors.Command}
	c.settings.Update(func() {})
}

func (c *controller) editCommand(command commands.Command) {
	c.formModel = &editors.Model{
		Open: true, Kind: editors.Command, ID: command.ID, Name: command.Name,
		Text: command.Text, Confirm: command.Confirm, Hotkey: command.Hotkey,
	}
	c.settings.Update(func() {})
}

func (c *controller) newQuicklink() {
	c.formModel = &editors.Model{Open: true, Kind: editors.Quicklink, ShowRoot: true}
	c.settings.Update(func() {})
}

func (c *controller) editQuicklink(link quicklink.Link) {
	c.formModel = &editors.Model{
		Open: true, Kind: editors.Quicklink, ID: link.ID, Name: link.Name,
		Text: link.Destination, ShowRoot: link.ShowInRoot, Hotkey: link.Hotkey,
	}
	c.settings.Update(func() {})
}

func (c *controller) newSnippet() {
	c.formModel = &editors.Model{
		Open: true, Kind: editors.Snippet, Body: ui.NewTextBuffer(""),
		Enabled: true, ShowHud: false,
	}
	c.settings.Update(func() {})
}

func (c *controller) editSnippet(record snippet.Snippet) {
	c.formModel = &editors.Model{
		Open: true, Kind: editors.Snippet, ID: record.ID, Name: record.Name,
		Keyword: record.Keyword, Body: ui.NewTextBuffer(record.Body), Enabled: record.Enabled,
		ShowHud: record.ShowConfirmation, Hotkey: c.snippetHotkey(record.ID),
	}
	c.settings.Update(func() {})
}

func (c *controller) cancelWorkflowForm() {
	if c.formModel == nil {
		return
	}
	c.formModel.Open = false
	c.formModel = nil
	c.settings.Update(func() {})
}

func (c *controller) saveWorkflowForm() {
	model := c.formModel
	if model == nil {
		return
	}
	model.Error = ""
	if strings.TrimSpace(model.Name) == "" {
		model.Error = c.text("请填写名称", "A name is required")
		c.settings.Update(func() {})
		return
	}
	if model.Kind != editors.Snippet && strings.TrimSpace(model.Text) == "" {
		if model.Kind == editors.Command {
			model.Error = c.text("请填写命令", "A command line is required")
		} else {
			model.Error = c.text("请填写目标地址", "A destination is required")
		}
		c.settings.Update(func() {})
		return
	}
	var err error
	switch model.Kind {
	case editors.Command:
		err = c.saveCommandForm(model)
	case editors.Quicklink:
		err = c.saveQuicklinkForm(model)
	case editors.Snippet:
		err = c.saveSnippetForm(model)
	}
	if err != nil {
		model.Error = err.Error()
		c.settings.Update(func() {})
		return
	}
	model.Open = false
	c.formModel = nil
	c.refreshExtras()
	c.reconcileItemHotkeys()
	c.settingsStatus = c.text("已保存", "Saved")
	c.settings.Update(func() {})
}

func (c *controller) saveCommandForm(model *editors.Model) error {
	if c.commands == nil {
		return errors.New(c.text("命令库不可用", "The command library is unavailable"))
	}
	_, err := c.commands.Upsert(commands.Command{
		ID: model.ID, Name: model.Name, Text: model.Text, Confirm: model.Confirm, Hotkey: model.Hotkey,
	})
	return err
}

func (c *controller) saveQuicklinkForm(model *editors.Model) error {
	if c.quicklinks == nil {
		return errors.New(c.text("快捷链接库不可用", "The quicklink library is unavailable"))
	}
	link := quicklink.Link{
		ID: model.ID, Name: model.Name, Destination: model.Text,
		ShowInRoot: model.ShowRoot, Hotkey: model.Hotkey,
	}
	if model.ID != "" {
		if existing, ok := c.quicklinks.Get(model.ID); ok {
			link.Pinned, link.PinOrder = existing.Pinned, existing.PinOrder
		}
	}
	_, err := c.quicklinks.Upsert(link)
	return err
}

func (c *controller) saveSnippetForm(model *editors.Model) error {
	if c.snippets == nil {
		return errors.New(c.text("片段库不可用", "The snippet library is unavailable"))
	}
	body := model.Body.String()
	if model.ID == "" {
		created, err := c.snippets.Create(model.Name, model.Keyword, model.Enabled, model.ShowHud, body)
		if err != nil {
			return err
		}
		c.setSnippetHotkey(created.ID, model.Hotkey)
		c.reloadSnippets()
		return nil
	}
	record, ok := c.snippetByPath(model.ID)
	if !ok {
		return errors.New(c.text("片段文件已不存在；取消后重新打开", "The snippet file no longer exists; cancel and reopen"))
	}
	record.Name = model.Name
	record.Keyword = model.Keyword
	record.Enabled = model.Enabled
	record.ShowConfirmation = model.ShowHud
	record.Body = body
	if _, err := c.snippets.Save(record); err != nil {
		if errors.Is(err, snippets.ErrConflict) {
			return errors.New(c.text("文件已在磁盘上更改；未覆盖，请取消后重新打开", "The file changed on disk; it was not overwritten. Cancel and reopen."))
		}
		if errors.Is(err, snippets.ErrFileNotFound) {
			return errors.New(c.text("片段文件已不存在；取消后重新打开", "The snippet file no longer exists; cancel and reopen"))
		}
		return err
	}
	c.setSnippetHotkey(record.ID, model.Hotkey)
	c.reloadSnippets()
	return nil
}

func (c *controller) deleteWorkflowForm() {
	model := c.formModel
	if model == nil || model.ID == "" {
		return
	}
	c.confirmWorkflowDelete(model.Kind, model.ID, model.Name)
}

func (c *controller) confirmWorkflowDelete(kind editors.Kind, id, name string) {
	if c.workflowDeleteConfirm {
		return
	}
	c.workflowDeleteConfirm = true
	parent := c.settings
	if parent == nil {
		parent = c.panel
	}
	options := mygo.MessageOptions{
		Parent: parent, Type: mygo.MessageWarning,
		Title:         c.text("删除", "Delete"),
		Message:       name,
		Detail:        c.text("此操作会立即从库中删除，无法撤销。", "This removes the item from the library immediately and cannot be undone."),
		Buttons:       []string{c.text("取消", "Cancel"), c.text("删除", "Delete")},
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
			c.workflowDeleteConfirm = false
			if c.closing || err != nil || result.Button != 1 {
				return
			}
			c.deleteWorkflowItem(kind, id)
		})
	}()
}

func (c *controller) deleteWorkflowItem(kind editors.Kind, id string) {
	var err error
	switch kind {
	case editors.Command:
		if c.commands != nil {
			err = c.commands.Remove(id)
		}
	case editors.Quicklink:
		if c.quicklinks != nil {
			err = c.quicklinks.Remove(id)
		}
	case editors.Snippet:
		if c.snippets != nil {
			record, ok := c.snippetByPath(id)
			if !ok {
				err = errors.New("snippet missing")
			} else if err = c.snippets.Delete(record); err == nil {
				c.removeSnippetHotkey(id)
				c.reloadSnippets()
			}
		}
	}
	if err != nil {
		c.settingsStatus = c.text("删除失败；原文件已保留", "Delete failed; the file was kept")
		log.Printf("workflow delete failed: %v", err)
	} else {
		c.settingsStatus = c.text("已删除", "Deleted")
	}
	if c.formModel != nil && c.formModel.ID == id {
		c.formModel = nil
	}
	c.refreshExtras()
	c.reconcileItemHotkeys()
	c.settings.Update(func() {})
}

// setSnippetFeature confirms the listening consent before enabling snippets.
func (c *controller) setSnippetFeature(enabled bool) {
	if !enabled {
		c.setFeature("snippetsEnabled", false)
		return
	}
	if c.featureSnippets || c.snippetConsent {
		return
	}
	c.snippetConsent = true
	parent := c.settings
	if parent == nil {
		parent = c.panel
	}
	options := mygo.MessageOptions{
		Parent: parent, Type: mygo.MessageQuestion,
		Title:         c.text("启用片段？", "Enable snippets?"),
		Message:       c.text("启用后 Wincast 会监听键入的关键词，并在本机展开片段。", "Wincast will listen for typed keywords and expand snippets locally."),
		Detail:        c.text("不保存按键记录，也不会上传内容。关闭功能会立即停止监听。", "No key log is kept and nothing is uploaded. Turning the feature off stops the listener."),
		Buttons:       []string{c.text("取消", "Cancel"), c.text("启用", "Enable")},
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
			c.snippetConsent = false
			if c.closing || err != nil || result.Button != 1 {
				if c.settings != nil {
					c.settings.Update(func() {})
				}
				return
			}
			c.setFeature("snippetsEnabled", true)
			if c.settings != nil {
				c.settings.Update(func() {})
			}
		})
	}()
}

// reconcileItemHotkeys re-registers the per-item shortcuts of enabled
// features. Registration failures are reported and never silently reassigned.
func (c *controller) reconcileItemHotkeys() {
	for accelerator := range c.itemHotkeys {
		mygo.GlobalShortcut.Unregister(accelerator)
	}
	c.itemHotkeys = make(map[string]bool, 8)
	if c.panel == nil || c.closing {
		return
	}
	register := func(accelerator string, entry domain.Application) {
		accelerator = strings.TrimSpace(accelerator)
		if accelerator == "" || c.itemHotkeys[accelerator] {
			return
		}
		boundEntry := entry
		if err := mygo.GlobalShortcut.Register(accelerator, func() {
			c.panel.Update(func() {
				if !c.closing {
					c.open(boundEntry)
				}
			})
		}); err != nil {
			log.Printf("item shortcut unavailable entry=%q error=%v", entry.Name, err)
			return
		}
		c.itemHotkeys[accelerator] = true
	}
	if c.featureCommands && c.commands != nil {
		for _, command := range c.commands.Commands() {
			register(command.Hotkey, domain.Application{ID: commands.EntryID(command.ID), Name: command.Name, Kind: domain.KindCommand})
		}
	}
	if c.featureQuicklinks && c.quicklinks != nil {
		for _, link := range c.quicklinks.Links() {
			register(link.Hotkey, domain.Application{ID: quicklink.EntryID(link.ID), Name: link.Name, Kind: domain.KindQuicklink})
		}
	}
	if c.featureSnippets {
		for _, record := range c.snippetSnapshot.Records {
			if !record.Enabled {
				continue
			}
			register(c.snippetHotkey(record.ID), domain.Application{ID: snippet.EntryID(record.ID), Name: record.Name, Kind: domain.KindSnippet})
		}
	}
}

// snippetHotkeys maps a snippet file path to its global shortcut. Markdown
// stays free of launcher metadata; the binding lives with the settings.
func (c *controller) snippetHotkeys() map[string]string {
	document, _ := c.settingDocument()
	if document == nil {
		return nil
	}
	raw, ok := document.Raw("snippetHotkeys")
	if !ok {
		return nil
	}
	bindings := make(map[string]string)
	if err := json.Unmarshal(raw, &bindings); err != nil {
		return nil
	}
	return bindings
}

func (c *controller) snippetHotkey(id string) string {
	return c.snippetHotkeys()[id]
}

func (c *controller) setSnippetHotkey(id, accelerator string) {
	document, writable := c.settingDocument()
	if document == nil || !writable {
		return
	}
	bindings := c.snippetHotkeys()
	if bindings == nil {
		bindings = make(map[string]string)
	}
	accelerator = strings.TrimSpace(accelerator)
	if accelerator == "" {
		delete(bindings, id)
	} else {
		bindings[id] = accelerator
	}
	if len(bindings) == 0 {
		document.Delete("snippetHotkeys")
	} else {
		_ = document.Set("snippetHotkeys", bindings)
	}
	c.queueSettingsSave()
}

func (c *controller) removeSnippetHotkey(id string) { c.setSnippetHotkey(id, "") }
