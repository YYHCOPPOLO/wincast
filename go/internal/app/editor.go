package app

import (
	"encoding/json"
	"errors"
	"log"
	"os"
	"time"
	"unicode/utf8"

	notesfeature "github.com/YYHCOPPOLO/wincast/go/internal/features/notes"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	notesui "github.com/YYHCOPPOLO/wincast/go/internal/ui/notes"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

type notesGeometry struct {
	X      int  `json:"x"`
	Y      int  `json:"y"`
	Width  int  `json:"width"`
	Height int  `json:"height"`
	Valid  bool `json:"valid"`
}

type notesWindow struct {
	window    *mygo.Window
	view      *notesui.View
	note      storage.Note
	summaries []storage.NoteSummary
	active    bool
	saveGen   uint64
	conflict  bool
}

const (
	notesDefaultWidth  = 560
	notesDefaultHeight = 460
	notesSaveDelay     = 300 * time.Millisecond
)

// openEditor creates or shows the one notes window. The body stays on the UI
// thread; only complete files reach the repository.
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
		c.createNotesWindow()
	}
	c.showNotesWindow()
}

func (c *controller) createNotesWindow() {
	editor := &notesWindow{}
	editor.view = notesui.New(c.language == "zh", notesui.Callbacks{
		Search:     c.noteSearch,
		Select:     c.loadNote,
		Create:     c.createNote,
		Delete:     c.confirmDeleteNote,
		Save:       func() { c.saveNote(false) },
		OpenFolder: c.openNotesFolder,
		Hide:       c.hideEditor,
		Edited:     c.noteEdited,
	})
	geometry := c.loadNotesGeometry()
	editor.window = mygo.NewWindow(mygo.WindowOptions{
		Title:           "Tinycast Go Preview Notes",
		Width:           geometry.Width,
		Height:          geometry.Height,
		Hidden:          true,
		BackgroundColor: "light-dark(#ffffff, #232327)",
		Content:         ui.View(editor.view.Render),
	})
	editor.window.OnClose(func(event *mygo.CloseEvent) {
		if c.closing {
			return
		}
		event.PreventDefault()
		c.hideEditor()
	})
	editor.window.OnFocus(func() {
		if c.closing || c.editor != editor || editor.conflict {
			return
		}
		c.checkNoteDiskChange()
	})
	editor.window.OnClosed(func() {
		if c.editor == editor {
			c.editor = nil
		}
	})
	c.editor = editor
}

func (c *controller) showNotesWindow() {
	editor := c.editor
	if editor == nil {
		return
	}
	c.reloadNotes()
	c.loadActiveNote()
	geometry := c.loadNotesGeometry()
	showWindow(editor.window)
	if geometry.Valid {
		editor.window.SetSize(geometry.Width, geometry.Height)
		editor.window.SetPosition(geometry.X, geometry.Y)
	} else {
		editor.window.Center()
	}
}

func (c *controller) hideEditor() {
	editor := c.editor
	if editor == nil {
		return
	}
	if c.closing {
		editor.window.Hide()
		return
	}
	if !c.saveNote(false) && editor.view.Dirty {
		// A conflict keeps the window open so the user can decide.
		editor.window.Update(func() {})
		return
	}
	c.persistNotesState()
	editor.window.Hide()
}

func (c *controller) noteSearch(query string) []notesui.Row {
	filtered := notesfeature.Filter(c.editorSummaries(), query)
	rows := make([]notesui.Row, len(filtered))
	for index, summary := range filtered {
		rows[index] = notesui.Row{ID: summary.ID, Title: summary.Title}
	}
	return rows
}

func (c *controller) editorSummaries() []storage.NoteSummary {
	if c.editor == nil {
		return nil
	}
	return c.editor.summaries
}

func (c *controller) reloadNotes() {
	editor := c.editor
	if editor == nil {
		return
	}
	list, err := c.notes.List(c.ctx)
	if err != nil {
		editor.view.Status = c.text("无法读取笔记目录", "Could not read the notes folder")
		log.Printf("notes list failed: %v", err)
		return
	}
	editor.summaries = list
}

// loadActiveNote selects the persisted note, else the most recent one.
func (c *controller) loadActiveNote() {
	editor := c.editor
	if editor == nil {
		return
	}
	preferred := ""
	if c.settingsDocument != nil {
		preferred = c.settingsDocument.String("notesActiveID", "")
	}
	if preferred != "" {
		for _, summary := range editor.summaries {
			if summary.ID == preferred {
				c.loadNote(summary.ID)
				return
			}
		}
	}
	if len(editor.summaries) > 0 {
		c.loadNote(editor.summaries[0].ID)
		return
	}
	editor.active = false
	editor.view.ClearNote()
	editor.view.Status = ""
}

func (c *controller) loadNote(id string) { c.loadNoteWith(id, true) }

func (c *controller) loadNoteWith(id string, flush bool) {
	editor := c.editor
	if editor == nil {
		return
	}
	if flush && editor.active && !c.saveNote(false) {
		return
	}
	note, err := c.notes.Load(c.ctx, id)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			editor.view.Status = c.text("笔记已在磁盘上重命名或删除", "The note was renamed or deleted on disk")
			c.reloadNotes()
			if len(editor.summaries) > 0 {
				c.loadNote(editor.summaries[0].ID)
			} else {
				editor.active = false
				editor.view.ClearNote()
			}
			return
		}
		editor.view.Status = c.text("无法读取笔记", "Could not read the note")
		log.Printf("note load failed: %v", err)
		return
	}
	editor.note = note
	editor.active = true
	editor.view.SetNote(note.Title, note.Body, utf8.RuneCountInString(note.Body))
	editor.view.Status = c.text("已保存", "Saved")
	editor.view.CloseSwitcher()
	c.persistActiveNote(id)
	editor.window.Update(func() {})
}

func (c *controller) noteEdited() {
	editor := c.editor
	if editor == nil || !editor.active {
		return
	}
	editor.view.Dirty = true
	editor.view.Count = utf8.RuneCountInString(editor.view.Body.String())
	editor.view.Status = c.text("未保存", "Unsaved")
	editor.saveGen++
	generation := editor.saveGen
	time.AfterFunc(notesSaveDelay, func() {
		if c.panel == nil {
			return
		}
		c.panel.Update(func() {
			if c.closing || c.editor != editor || generation != editor.saveGen {
				return
			}
			c.saveNote(false)
			editor.window.Update(func() {})
		})
	})
}

// saveNote writes the draft. It returns true when the note is on disk: an
// unchanged body, a successful save, or nothing to save.
func (c *controller) saveNote(force bool) bool {
	editor := c.editor
	if editor == nil || !editor.active {
		return true
	}
	body := editor.view.Body.String()
	if body == editor.note.Body && !editor.view.Dirty {
		return true
	}
	note := editor.note
	note.Body = body
	var saved storage.Note
	var err error
	if force {
		saved, err = c.notes.ForceSave(c.ctx, note)
	} else {
		saved, err = c.notes.Save(c.ctx, note)
	}
	if err != nil {
		if errors.Is(err, storage.ErrNoteConflict) {
			editor.view.Status = c.text("笔记已在磁盘上更改，未覆盖", "The note changed on disk; the external edit is kept")
			c.showNoteConflict()
			return false
		}
		editor.view.Status = c.text("保存失败；草稿保留", "Save failed; the draft is kept")
		log.Printf("note save failed: %v", err)
		return false
	}
	editor.note = saved
	editor.view.Dirty = false
	editor.view.Count = utf8.RuneCountInString(body)
	editor.view.Status = c.text("已保存", "Saved")
	c.reloadNotes()
	return true
}

// checkNoteDiskChange picks up external edits: a clean draft reloads, a dirty
// draft asks before either version is replaced.
func (c *controller) checkNoteDiskChange() {
	editor := c.editor
	if editor == nil || !editor.active {
		return
	}
	modified, err := c.notes.Modified(c.ctx, editor.note.ID)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			editor.view.Status = c.text("笔记已在磁盘上删除", "The note was deleted on disk")
			if !editor.view.Dirty {
				c.reloadNotes()
				if len(editor.summaries) > 0 {
					c.loadNote(editor.summaries[0].ID)
				} else {
					editor.active = false
					editor.view.ClearNote()
				}
			}
		}
		return
	}
	if modified.Equal(editor.note.Modified) {
		return
	}
	if !editor.view.Dirty {
		c.loadNote(editor.note.ID)
		editor.view.Status = c.text("已载入磁盘上的更改", "Loaded the change on disk")
		return
	}
	editor.view.Status = c.text("笔记已在磁盘上更改", "The note changed on disk")
	c.showNoteConflict()
}

func (c *controller) showNoteConflict() {
	editor := c.editor
	if editor == nil || editor.conflict {
		return
	}
	editor.conflict = true
	options := mygo.MessageOptions{
		Parent: editor.window, Type: mygo.MessageWarning,
		Title:         c.text("笔记已在磁盘上更改", "The note changed on disk"),
		Message:       c.text("要保留哪一份内容？", "Which version should be kept?"),
		Detail:        c.text("覆盖会替换磁盘上的外部修改。", "Overwriting replaces the external edits on disk."),
		Buttons:       []string{c.text("取消", "Cancel"), c.text("重新加载", "Reload"), c.text("覆盖磁盘", "Overwrite")},
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
			if c.editor != editor {
				return
			}
			editor.conflict = false
			if c.closing || err != nil {
				return
			}
			switch result.Button {
			case 1:
				editor.view.Dirty = false
				editor.saveGen++
				c.loadNoteForce(editor.note.ID)
			case 2:
				c.saveNote(true)
			}
			editor.window.Update(func() {})
		})
	}()
}

// loadNoteForce reloads the disk version, discarding the in-memory draft
// after the user explicitly chose it.
func (c *controller) loadNoteForce(id string) {
	editor := c.editor
	if editor == nil {
		return
	}
	editor.view.Dirty = false
	c.loadNoteWith(id, false)
}

func (c *controller) createNote() {
	editor := c.editor
	if editor == nil {
		return
	}
	if editor.active && !c.saveNote(false) {
		return
	}
	note, err := c.notes.Create(c.ctx)
	if err != nil {
		editor.view.Status = c.text("无法新建笔记", "Could not create a note")
		log.Printf("note create failed: %v", err)
		return
	}
	editor.note = note
	editor.active = true
	editor.view.SetNote(note.Title, note.Body, 0)
	editor.view.Status = c.text("已新建", "Created")
	editor.view.CloseSwitcher()
	c.reloadNotes()
	c.persistActiveNote(note.ID)
	editor.window.Update(func() {})
}

func (c *controller) confirmDeleteNote(id string) {
	editor := c.editor
	if editor == nil || c.notesDeleteConfirm {
		return
	}
	c.notesDeleteConfirm = true
	options := mygo.MessageOptions{
		Parent: editor.window, Type: mygo.MessageWarning,
		Title:         c.text("删除这篇笔记？", "Delete this note?"),
		Message:       id,
		Detail:        c.text("文件会立即从笔记目录删除。", "The file is removed from the notes folder immediately."),
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
			c.notesDeleteConfirm = false
			if c.editor != editor || c.closing || err != nil || result.Button != 1 {
				return
			}
			c.deleteNote(id)
		})
	}()
}

func (c *controller) deleteNote(id string) {
	editor := c.editor
	if editor == nil {
		return
	}
	if err := c.notes.Delete(c.ctx, id); err != nil {
		editor.view.Status = c.text("无法删除笔记", "Could not delete the note")
		log.Printf("note delete failed: %v", err)
		return
	}
	c.reloadNotes()
	next, found := notesfeature.Successor(editor.summaries, id)
	if editor.active && editor.note.ID == id {
		editor.active = false
		editor.view.Dirty = false
		editor.saveGen++
		if found {
			c.loadNoteForce(next.ID)
		} else {
			editor.view.ClearNote()
			editor.view.Status = c.text("笔记已删除", "Note deleted")
			c.persistActiveNote("")
		}
	} else if found {
		editor.view.Status = c.text("笔记已删除", "Note deleted")
	}
	editor.window.Update(func() {})
}

func (c *controller) openNotesFolder() {
	if c.notesRoot == "" {
		return
	}
	root := c.notesRoot
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		if err := mygo.Shell.OpenPath(root); err != nil {
			log.Printf("open notes folder failed: %v", err)
		}
	}()
}

func (c *controller) loadNotesGeometry() notesGeometry {
	geometry := notesGeometry{Width: notesDefaultWidth, Height: notesDefaultHeight}
	document, writable := c.settingDocument()
	if document == nil || !writable {
		return geometry
	}
	raw, ok := document.Raw("notesWindow")
	if !ok {
		return geometry
	}
	if err := json.Unmarshal(raw, &geometry); err != nil {
		return notesGeometry{Width: notesDefaultWidth, Height: notesDefaultHeight}
	}
	if geometry.Width < 320 || geometry.Height < 260 || geometry.Width > 4000 || geometry.Height > 3000 {
		geometry.Width, geometry.Height = notesDefaultWidth, notesDefaultHeight
		geometry.Valid = false
	}
	return geometry
}

func (c *controller) persistNotesState() {
	editor := c.editor
	document, writable := c.settingDocument()
	if editor == nil || document == nil || !writable {
		return
	}
	width, height := editor.window.Size()
	x, y := editor.window.Position()
	geometry := notesGeometry{X: x, Y: y, Width: width, Height: height, Valid: true}
	_ = document.Set("notesWindow", geometry)
	if editor.active {
		_ = document.Set("notesActiveID", editor.note.ID)
	}
	c.queueSettingsSave()
}

func (c *controller) persistActiveNote(id string) {
	document, writable := c.settingDocument()
	if document == nil || !writable {
		return
	}
	if id == "" {
		document.Delete("notesActiveID")
	} else {
		_ = document.Set("notesActiveID", id)
	}
	c.queueSettingsSave()
}
