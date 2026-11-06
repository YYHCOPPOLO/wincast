package app

import (
	"errors"
	"io/fs"
	"log"

	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

type editorWindow struct {
	window *mygo.Window
	buffer *ui.TextBuffer
	note   storage.Note
	status string
	loaded bool
}

// openEditor owns one separate native window. Its text stays on the UI
// thread; only the finished Markdown body reaches the repository.
func (c *controller) openEditor() {
	if c.closing {
		return
	}
	if !c.featureNotes {
		c.view.Status = c.view.Text("笔记功能已关闭", "Notes are disabled")
		c.panel.Update(func() {})
		return
	}
	c.hide()
	if c.editor == nil {
		editor := &editorWindow{buffer: ui.NewTextBuffer("")}
		c.editor = editor
		editor.window = mygo.NewWindow(mygo.WindowOptions{
			Title:           "Tinycast Go Preview Notes",
			Width:           520,
			Height:          440,
			Hidden:          true,
			BackgroundColor: "light-dark(#ffffff, #232327)",
			Content:         ui.View(c.editorView),
		})
		editor.window.OnClose(func(event *mygo.CloseEvent) {
			if c.closing {
				return
			}
			if !c.saveEditor() {
				event.PreventDefault()
				editor.window.Update(func() {})
			}
		})
		editor.window.OnClosed(func() { c.editor = nil })
	}
	if !c.editor.loaded {
		c.loadScratch()
	}
	showWindow(c.editor.window)
}

func (c *controller) loadScratch() {
	editor := c.editor
	if editor == nil {
		return
	}
	note, err := c.notes.Load(c.ctx, "scratch")
	if errors.Is(err, fs.ErrNotExist) {
		note = storage.Note{ID: "scratch", Title: "Scratch", Body: "# Scratch\n\n"}
		note, err = c.notes.Save(c.ctx, note)
	}
	if err != nil {
		editor.loaded = true
		editor.status = c.text("无法读取笔记；原文件未改动", "Could not read the note; the file was left untouched")
		log.Printf("note load failed: %v", err)
		return
	}
	editor.note = note
	editor.buffer.Set(note.Body)
	editor.loaded = true
}

func (c *controller) saveEditor() bool {
	editor := c.editor
	if editor == nil {
		return true
	}
	body := editor.buffer.String()
	if body == editor.note.Body {
		return true
	}
	note := editor.note
	note.Body = body
	saved, err := c.notes.Save(c.ctx, note)
	if err != nil {
		if errors.Is(err, storage.ErrNoteConflict) {
			editor.status = c.text("笔记已在磁盘上更改；未覆盖外部版本", "The note changed on disk; the external version was kept")
		} else {
			editor.status = c.text("保存失败；未写入半文件", "Save failed; no partial file was written")
		}
		log.Printf("note save failed: %v", err)
		return false
	}
	editor.note = saved
	editor.status = c.text("已保存", "Saved")
	log.Printf("note saved bytes=%d", len(body))
	return true
}

func (c *controller) editorView(ctx *ui.Context) {
	tokens := theme.For(c.dark())
	ctx.SetTheme(tokens.Theme())
	ctx.Root().Background(tokens.Surface)
	editor := c.editor
	if editor == nil {
		return
	}
	title := editor.note.Title
	if title == "" {
		title = c.text("笔记", "Notes")
	}
	status := editor.status
	if status == "" {
		if editor.buffer.String() != editor.note.Body {
			status = c.text("未保存", "Unsaved")
		} else {
			status = c.text("已保存", "Saved")
		}
	}
	ui.Column(ctx).Fill().Children(func() {
		ui.Row(ctx).Padding(12, 16).Gap(12).AlignItems(ui.Center).Children(func() {
			ui.Text(ctx, title).FontSize(14).Bold().Grow(1).SingleLine()
			ui.Text(ctx, status).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Role(ui.RoleStatus)
			if ui.Button(ctx, c.text("保存", "Save")).Clicked() {
				c.saveEditor()
				editor.window.Update(func() {})
			}
		})
		ui.Divider(ctx)
		ui.TextAreaBufferBase(ctx, editor.buffer).Grow(1).Padding(14, 16).FontSize(14).
			Label(c.text("笔记内容", "Note content"))
		ui.Divider(ctx)
		ui.Text(ctx, c.text("关闭窗口时保存；外部修改不会被覆盖。", "Saves when the window closes; external edits are never overwritten.")).
			FontSize(theme.KeyCapFont).TextColor(tokens.TextTertiary).Padding(8, 16)
	})
}
