package app

import (
	"context"
	"errors"
	"log"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/snippet"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/template"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/egoist/mygo"
)

// reloadSnippets lists the library. Every show re-lists, so files added or
// renamed outside Tinycast appear without a watcher.
func (c *controller) reloadSnippets() {
	if c.snippets == nil {
		return
	}
	snapshot, err := c.snippets.Load()
	if err != nil {
		c.snippetError = c.text("无法读取片段库", "Could not read the snippet library")
		log.Printf("snippet library load failed: %v", err)
		return
	}
	c.snippetSnapshot = snapshot
	c.snippetError = ""
	for _, issue := range snapshot.Issues {
		log.Printf("snippet issue path=%q error=%q", issue.Path, issue.Message)
	}
}

func (c *controller) snippetList() []template.Snippet {
	records := c.snippetSnapshot.Records
	pool := make([]template.Snippet, 0, len(records))
	for _, record := range records {
		pool = append(pool, template.Snippet{
			ID: record.ID, Name: record.Name, Keyword: record.Keyword,
			Enabled: record.Enabled, Body: record.Body,
		})
	}
	return pool
}

func (c *controller) snippetByPath(path string) (snippet.Snippet, bool) {
	for _, record := range c.snippetSnapshot.Records {
		if record.ID == path {
			return record, true
		}
	}
	// A rename or delete outside Tinycast changes identity. Re-list once so a
	// fresh file with the same path can still be found.
	c.reloadSnippets()
	for _, record := range c.snippetSnapshot.Records {
		if record.ID == path {
			return record, true
		}
	}
	return snippet.Snippet{}, false
}

func (c *controller) startSnippet(application domain.Application) {
	path, ok := snippet.IDFromEntry(application.ID)
	if !ok {
		return
	}
	record, found := c.snippetByPath(path)
	if !found {
		c.view.Status = c.view.Text("片段已不存在", "The snippet no longer exists")
		c.panel.Update(func() {})
		return
	}
	c.entryContextAsync(application, template.UsesSelection(record.Body), func(captured template.Context) {
		output := template.ExpandSnippet(record.Body, captured, nil, c.snippetList(), record.ID)
		if len(output.Arguments) > 0 {
			c.pendingArgs = &argumentSession{entry: application, ctx: captured, specs: output.Arguments}
			c.view.SetExpanded(true)
			if c.view.SetModeArguments(application, output.Arguments) {
				c.showPanel(false)
			}
			return
		}
		c.deliverSnippet(record, output)
	})
}

func (c *controller) submitSnippetArguments(session *argumentSession, args map[string]string) {
	path, ok := snippet.IDFromEntry(session.entry.ID)
	if !ok {
		return
	}
	record, found := c.snippetByPath(path)
	if !found {
		return
	}
	output := template.ExpandSnippet(record.Body, session.ctx, args, c.snippetList(), record.ID)
	if len(output.Arguments) > 0 {
		session.specs = output.Arguments
		c.pendingArgs = session
		c.view.SetModeArguments(session.entry, output.Arguments)
		c.panel.Update(func() {})
		return
	}
	c.deliverSnippet(record, output)
}

// deliverSnippet copies the expansion and pastes it into the remembered
// target. Failure keeps the text on the clipboard for manual paste.
func (c *controller) deliverSnippet(record snippet.Snippet, output template.Output) {
	text := output.Text
	if text == "" {
		c.view.Status = c.view.Text("片段内容为空", "The snippet is empty")
		return
	}
	if err := windows.WriteClipboardText(c.panel.NativeHandle(), text, true); err != nil {
		c.view.Status = c.view.Text("复制失败，请重试", "Could not copy; please retry")
		log.Printf("snippet copy failed: %v", err)
		return
	}
	left := 0
	if output.HasCursor {
		if total := template.GraphemeCount(text); output.Cursor < total {
			left = total - output.Cursor
		}
	}
	target := c.target
	if err := target.Validate(); err != nil {
		c.hide()
		c.view.Status = c.view.Text("已复制；原窗口已不可用", "Copied; the previous window is unavailable")
		c.logSnippetDelivery(record, false)
		return
	}
	sequence := windows.ClipboardSequence()
	owner := c.panel.NativeHandle()
	c.hide()
	if err := target.Restore(owner); err != nil {
		c.view.Status = c.view.Text("已复制；无法恢复原窗口或权限不足", "Copied; the previous window could not be restored or requires permission")
		log.Printf("snippet paste foreground failed: %v", err)
		c.logSnippetDelivery(record, false)
		return
	}
	ctx, cancel := context.WithTimeout(c.ctx, 800*time.Millisecond)
	c.pasteCancel = cancel
	c.pasteID++
	id := c.pasteID
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		defer cancel()
		err := target.Paste(ctx, sequence)
		if err == nil && left > 0 {
			time.Sleep(60 * time.Millisecond)
			err = windows.SendLeftArrows(left)
		}
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing || id != c.pasteID {
				return
			}
			c.pasteCancel = nil
			if err != nil {
				if errors.Is(err, windows.ErrPasteUnconfirmed) {
					c.view.Status = c.view.Text("已复制；粘贴状态未确认，请检查原窗口", "Copied; paste status is unconfirmed, please check the previous window")
				} else {
					c.view.Status = c.view.Text("粘贴未发送；内容已复制，可手动粘贴", "Paste was not sent; the text is copied for manual paste")
				}
				log.Printf("snippet paste failed: %v", err)
				c.logSnippetDelivery(record, false)
				return
			}
			c.view.Status = c.view.Text("已粘贴片段", "Snippet pasted")
			c.logSnippetDelivery(record, true)
			if record.ShowConfirmation {
				c.showToast(c.view.Text("已展开片段："+record.Name, "Snippet expanded: "+record.Name))
			}
		})
	}()
}

func (c *controller) logSnippetDelivery(record snippet.Snippet, delivered bool) {
	log.Printf("snippet delivered=%t name=%q", delivered, record.Name)
}

// showToast reports completion without activating Tinycast. A native
// notification is used because the palette is hidden after delivery.
func (c *controller) showToast(message string) {
	if c.closing || strings.TrimSpace(message) == "" {
		return
	}
	if !mygo.NotificationsSupported() {
		return
	}
	notification := mygo.NewNotification(mygo.NotificationOptions{Title: "Tinycast", Body: message, Silent: true})
	if notification == nil {
		return
	}
	if err := notification.Show(); err != nil {
		log.Printf("completion notification unavailable: %v", err)
		return
	}
	time.AfterFunc(1600*time.Millisecond, notification.Close)
}
