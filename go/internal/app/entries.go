package app

import (
	"context"
	"log"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/quicklink"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/snippet"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/template"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/clipboard"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/commands"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/quicklinks"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/egoist/mygo"
)

// argumentSession captures the expansion environment once, before the first
// prompt, so clipboard, selection and clock cannot drift while the form is
// open.
type argumentSession struct {
	entry domain.Application
	ctx   template.Context
	specs []template.Argument
}

// refreshExtras rebuilds the command, quicklink and snippet entries that the
// launcher ranks next to applications. Stores are tiny JSON libraries, so the
// rebuild runs on the UI thread.
func (c *controller) refreshExtras() {
	if c.launcher == nil || c.view == nil {
		return
	}
	extras := make([]domain.Application, 0, 16)
	keywords := make(map[string]string, 8)
	if c.featureCommands && c.commandsShowInLauncher && c.commands != nil && c.commands.Available() {
		for _, command := range c.commands.Commands() {
			extras = append(extras, domain.Application{ID: commands.EntryID(command.ID), Name: command.Name, Kind: domain.KindCommand})
		}
	}
	if c.featureQuicklinks && c.quicklinksShowInLauncher && c.quicklinks != nil && c.quicklinks.Available() {
		for _, link := range c.quicklinks.Links() {
			if !link.ShowInRoot {
				continue
			}
			extras = append(extras, domain.Application{ID: quicklink.EntryID(link.ID), Name: link.Name, Kind: domain.KindQuicklink})
		}
	}
	if c.featureSnippets && c.snippetsShowInLauncher && c.snippets != nil {
		for _, record := range c.snippetSnapshot.Records {
			if !record.Enabled {
				continue
			}
			entry := domain.Application{ID: snippet.EntryID(record.ID), Name: record.Name, Kind: domain.KindSnippet}
			extras = append(extras, entry)
			keywords[entry.ID] = record.Keyword
		}
	}
	c.launcher.SetExtras(extras, keywords)
	if c.view.Mode == palette.ModeLauncher {
		c.view.Filter()
	}
}

// captureTemplateContext snapshots clipboard history and the clock. Selection
// capture is a separate asynchronous step because it needs the panel hidden
// and the target focused.
func (c *controller) captureTemplateContext() template.Context {
	context := template.Context{
		Now:    time.Now(),
		Locale: windows.UserLocale(),
		NewUUID: func() string {
			id, err := storage.NewID()
			if err != nil {
				return ""
			}
			return id
		},
	}
	if c.clipboard != nil {
		items := c.clipboard.Store().Search("", clipboard.FilterText)
		texts := make([]string, 0, min(len(items), 16))
		for _, item := range items {
			if item.Kind != clipboard.KindText || item.Text == "" {
				continue
			}
			texts = append(texts, item.Text)
			if len(texts) == 16 {
				break
			}
		}
		context.Clipboard = texts
	}
	return context
}

// entryContextAsync captures selected text for templates that read it, then
// continues on the UI thread. The panel is hidden for the copy and shown again
// when an argument prompt is required.
func (c *controller) entryContextAsync(entry domain.Application, needsSelection bool, next func(template.Context)) {
	captured := c.captureTemplateContext()
	if !needsSelection || c.target.Window == 0 {
		next(captured)
		return
	}
	c.hide()
	target := c.target
	owner := c.panel.NativeHandle()
	c.clipboardSuppress.Store(true)
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		defer func() {
			time.Sleep(150 * time.Millisecond)
			c.clipboardSuppress.Store(false)
		}()
		ctx, cancel := context.WithTimeout(c.ctx, time.Second)
		selection, err := windows.CaptureSelection(ctx, target, owner)
		cancel()
		if c.ctx.Err() != nil {
			return
		}
		if err != nil {
			log.Printf("selection capture unavailable: %v", err)
		}
		captured.Selection = selection
		c.panel.Update(func() {
			if c.closing {
				return
			}
			next(captured)
		})
	}()
}

func (c *controller) startCommand(application domain.Application) {
	if c.commands == nil {
		return
	}
	id, ok := commands.IDFromEntry(application.ID)
	if !ok {
		return
	}
	command, found := c.commands.Get(id)
	if !found {
		c.view.Status = c.view.Text("命令已不存在", "The command no longer exists")
		c.panel.Update(func() {})
		return
	}
	if command.Confirm {
		c.confirmCommand(command)
		return
	}
	c.runCommand(command)
}

func (c *controller) confirmCommand(command commands.Command) {
	if c.commandConfirm {
		return
	}
	c.commandConfirm = true
	c.hide()
	options := mygo.MessageOptions{
		Parent: c.panel, Type: mygo.MessageQuestion,
		Title:         command.Name,
		Message:       c.text("运行这个命令？", "Run this command?"),
		Detail:        command.Text,
		Buttons:       []string{c.text("取消", "Cancel"), c.text("运行", "Run")},
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
			c.commandConfirm = false
			if c.closing {
				return
			}
			if err != nil {
				log.Printf("command confirmation failed: %v", err)
				return
			}
			if result.Button == 1 {
				c.runCommand(command)
			}
		})
	}()
}

func (c *controller) runCommand(command commands.Command) {
	if c.closing {
		return
	}
	c.hide()
	log.Printf("command start name=%q", command.Name)
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		err := windows.RunCommandLine(c.ctx, command.Text)
		if c.ctx.Err() != nil {
			return
		}
		if err != nil {
			log.Printf("command failed name=%q error=%v", command.Name, err)
			c.panel.Update(func() {
				if c.closing {
					return
				}
				c.view.Status = c.view.Text("命令执行失败", "The command failed")
			})
			c.showFailure(c.text("命令执行失败", "The command failed"), command.Name, err.Error())
			return
		}
		log.Printf("command finished name=%q", command.Name)
	}()
}

// showFailure reports a bounded error through Tinycast's own dialog. Callers
// run it off the UI loop, matching the other native dialogs.
func (c *controller) showFailure(title, subject, detail string) {
	if c.closing {
		return
	}
	if len(detail) > 4096 {
		detail = detail[:4096]
	}
	options := mygo.MessageOptions{
		Parent:        c.panel,
		Type:          mygo.MessageError,
		Title:         title,
		Message:       subject,
		Detail:        detail,
		Buttons:       []string{c.text("好", "OK")},
		DefaultButton: 0, CancelButton: 0,
	}
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		if _, err := mygo.Dialog.Message(options); err != nil {
			log.Printf("failure dialog unavailable: %v", err)
		}
	}()
}

func (c *controller) startQuicklink(application domain.Application) {
	if c.quicklinks == nil {
		return
	}
	id, ok := quicklink.IDFromEntry(application.ID)
	if !ok {
		return
	}
	link, found := c.quicklinks.Get(id)
	if !found {
		c.view.Status = c.view.Text("快捷链接已不存在", "The quicklink no longer exists")
		c.panel.Update(func() {})
		return
	}
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
	}
	c.entryContextAsync(application, template.UsesSelection(link.Destination), func(context template.Context) {
		c.expandQuicklink(application, link, context)
	})
}

func (c *controller) expandQuicklink(application domain.Application, link quicklink.Link, context template.Context) {
	output := quicklink.Expand(link.Destination, context, nil)
	if len(output.Arguments) > 0 {
		c.pendingArgs = &argumentSession{entry: application, ctx: context, specs: output.Arguments}
		c.view.SetExpanded(true)
		if c.view.SetModeArguments(application, output.Arguments) {
			c.showPanel(false)
		}
		return
	}
	c.openQuicklink(link, output.Text)
}

func (c *controller) openQuicklink(link quicklink.Link, destination string) {
	c.hide()
	if strings.TrimSpace(destination) == "" {
		c.view.Status = c.view.Text("链接为空，未打开", "The link is empty; nothing was opened")
		c.showPanel(false)
		return
	}
	if err := quicklinks.Open(destination); err != nil {
		log.Printf("quicklink open failed name=%q error=%v", link.Name, err)
		c.view.Status = c.view.Text("无法打开快捷链接", "Could not open the quicklink")
		c.showPanel(false)
		c.showFailure(c.text("无法打开快捷链接", "Could not open the quicklink"), link.Name, err.Error())
		return
	}
	c.view.Status = c.view.Text("已打开快捷链接", "Quicklink opened")
}

// submitArguments continues the stored activation after the prompt commits.
func (c *controller) submitArguments(entry domain.Application, values []string) {
	session := c.pendingArgs
	c.pendingArgs = nil
	if session == nil || session.entry.ID != entry.ID || len(values) != len(session.specs) {
		return
	}
	args := make(map[string]string, len(values))
	for index, spec := range session.specs {
		args[spec.Name] = values[index]
	}
	switch entry.Kind {
	case domain.KindQuicklink:
		id, ok := quicklink.IDFromEntry(entry.ID)
		if !ok {
			return
		}
		link, found := c.quicklinks.Get(id)
		if !found {
			return
		}
		output := quicklink.Expand(link.Destination, session.ctx, args)
		if len(output.Arguments) > 0 {
			session.specs = output.Arguments
			c.pendingArgs = session
			c.view.SetModeArguments(entry, output.Arguments)
			c.panel.Update(func() {})
			return
		}
		c.openQuicklink(link, output.Text)
	case domain.KindSnippet:
		c.submitSnippetArguments(session, args)
	}
}
