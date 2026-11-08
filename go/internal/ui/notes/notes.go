// Package notes draws the floating notes window. It owns display state only;
// filesystem work stays in the app and storage layers.
package notes

import (
	"fmt"

	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
)

// Row is one switcher result.
type Row struct {
	ID    string
	Title string
}

type Callbacks struct {
	Search     func(query string) []Row
	Select     func(id string)
	Create     func()
	Delete     func(id string)
	Save       func()
	OpenFolder func()
	Hide       func()
	Edited     func()
}

// View is UI-thread-owned note state. Body is the canonical Markdown source.
type View struct {
	Chinese   bool
	Title     string
	Status    string
	Dirty     bool
	Count     int
	HasNote   bool
	Switcher  bool
	Query     string
	Results   []Row
	Chosen    int
	focusBody bool

	Body        *ui.TextBuffer
	queryHandle ui.Handle
	list        ui.ListState
	callbacks   Callbacks
}

func New(chinese bool, callbacks Callbacks) *View {
	return &View{Chinese: chinese, Body: ui.NewTextBuffer(""), Chosen: -1, callbacks: callbacks}
}

func (v *View) Text(zh, en string) string {
	if v.Chinese {
		return zh
	}
	return en
}

// SetNote replaces the literal source and clears the edit marker.
func (v *View) SetNote(title, body string, count int) {
	v.Title = title
	v.HasNote = true
	v.Dirty = false
	v.Count = count
	v.Body.Set(body)
	v.focusBody = true
}

func (v *View) ClearNote() {
	v.Title = ""
	v.HasNote = false
	v.Dirty = false
	v.Count = 0
	v.Body.Set("")
}

// OpenSwitcher lists notes and focuses the query field.
func (v *View) OpenSwitcher(rows []Row) {
	v.Switcher = true
	v.Query = ""
	v.Results = rows
	v.Chosen = -1
	if len(rows) > 0 {
		v.Chosen = 0
	}
	v.list.ScrollIntoView(0)
}

func (v *View) CloseSwitcher() { v.Switcher = false }

// FocusQuery enters the switcher search field after the frame that built it.
func (v *View) FocusQuery() { v.queryHandle.Focus() }

func (v *View) Render(c *ui.Context) {
	tokens := theme.ForContext(c)
	c.SetTheme(tokens.Theme())
	// The note switcher's ResultList temporarily changes the widget theme,
	// which resets the root too; the window surface must be applied last.
	defer c.Root().Background(tokens.PanelBackground(c.Vibrancy()))
	c.OnShortcut(ui.Ctrl, ui.KeyN, v.callbacks.Create)
	c.OnShortcut(ui.Ctrl, ui.KeyP, v.toggleSwitcher)
	c.OnShortcut(ui.Ctrl, ui.KeyO, v.callbacks.OpenFolder)
	c.OnShortcut(0, ui.KeyEscape, v.escape)
	c.OnShortcut(ui.Ctrl, ui.KeyS, v.callbacks.Save)

	ui.Column(c).Fill().Children(func() {
		ui.Row(c).Height(46).Padding(8, 14).Gap(10).AlignItems(ui.Center).Children(func() {
			if ui.Button(c, v.Text("新建", "New")).Label(v.Text("新建笔记", "New note")).Clicked() {
				v.callbacks.Create()
			}
			if ui.Button(c, v.Text("浏览", "Browse")).Label(v.Text("浏览笔记", "Browse notes")).Clicked() {
				v.openSwitcher()
			}
			if ui.Button(c, v.Text("文件夹", "Folder")).Label(v.Text("打开笔记文件夹", "Open notes folder")).Clicked() {
				v.callbacks.OpenFolder()
			}
			title := v.Title
			if title == "" {
				title = v.Text("笔记", "Notes")
			}
			if v.Dirty {
				title += " •"
			}
			ui.Text(c, title).FontSize(14).Bold().Grow(1).SingleLine().TextAlign(ui.Center)
			status := v.Status
			ui.Text(c, status).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).SingleLine().Role(ui.RoleStatus)
		})
		ui.Divider(c)
		if v.Switcher {
			v.switcher(c, tokens)
			return
		}
		if !v.HasNote {
			v.empty(c, tokens)
			return
		}
		editor := ui.TextAreaBufferBase(c, v.Body).Grow(1).Padding(14, 16).FontSize(14).
			TextColor(tokens.NoteText).SelectionColor(tokens.Selection).
			Label(v.Text("笔记内容", "Note content")).OnChange(v.edited)
		if v.focusBody {
			editor = editor.AutoFocus()
			v.focusBody = false
		}
		ui.Divider(c)
		ui.Row(c).Height(28).Padding(4, 12).Gap(8).AlignItems(ui.Center).Children(func() {
			ui.Text(c, v.countLabel()).FontSize(theme.KeyCapFont).TextColor(tokens.TextTertiary).Grow(1)
			ui.Text(c, v.Text("自动保存", "Autosaves")).FontSize(theme.KeyCapFont).TextColor(tokens.TextTertiary)
		})
	})
}

func (v *View) edited() { v.callbacks.Edited() }

func (v *View) switcher(c *ui.Context, tokens theme.Tokens) {
	ui.Row(c).Padding(8, 14).Gap(8).AlignItems(ui.Center).Children(func() {
		if ui.TextInput(c, &v.Query).Bind(&v.queryHandle).Grow(1).
			Placeholder(v.Text("搜索笔记…", "Search notes…")).
			Label(v.Text("搜索笔记", "Search notes")).Changed() {
			v.Results = v.callbacks.Search(v.Query)
			v.Chosen = -1
			if len(v.Results) > 0 {
				v.Chosen = 0
			}
			v.list.ScrollIntoView(0)
		}
	})
	ui.Divider(c)
	v.list.Selected = &v.Chosen
	v.list.Key = func(i int) any { return v.Results[i].ID }
	v.list.Label = func(i int) string { return v.Results[i].Title }
	list := components.ResultList(c, tokens, &v.list, len(v.Results), func(i int) {
		components.ResultRow(c, tokens, components.Row{
			Fallback: "N",
			Title:    v.Results[i].Title,
			Subtitle: v.Results[i].ID,
			Selected: i == v.Chosen,
		})
	}).Grow(1).Padding(0, 8).Label(v.Text("笔记列表", "Notes"))
	if list.Submitted() {
		v.choose(v.Chosen)
	}
	if v.Chosen >= 0 && v.Chosen < len(v.Results) {
		ui.Row(c).Padding(4, 14).Gap(8).Children(func() {
			id := v.Results[v.Chosen].ID
			if ui.Button(c, v.Text("删除", "Delete")).Clicked() {
				v.callbacks.Delete(id)
			}
			if ui.Button(c, v.Text("打开", "Open")).Clicked() {
				v.choose(v.Chosen)
			}
		})
	}
	if len(v.Results) == 0 {
		ui.Text(c, v.Text("没有匹配的笔记", "No matching notes")).TextColor(tokens.TextTertiary).Padding(20)
	}
}

func (v *View) empty(c *ui.Context, tokens theme.Tokens) {
	ui.Column(c).Grow(1).AlignItems(ui.Center).AlignContent(ui.Center).Gap(10).Children(func() {
		ui.Text(c, v.Text("还没有笔记", "No notes yet")).FontSize(16).TextColor(tokens.TextSecondary)
		if ui.Button(c, v.Text("新建笔记", "Create note")).Clicked() {
			v.callbacks.Create()
		}
	})
}

func (v *View) choose(index int) {
	if index < 0 || index >= len(v.Results) {
		return
	}
	v.callbacks.Select(v.Results[index].ID)
}

func (v *View) openSwitcher() {
	if v.callbacks.Search == nil {
		return
	}
	v.OpenSwitcher(v.callbacks.Search(""))
	v.queryHandle.Focus()
}

func (v *View) toggleSwitcher() {
	if v.Switcher {
		v.CloseSwitcher()
		return
	}
	v.openSwitcher()
}

func (v *View) escape() {
	if v.Switcher {
		v.Switcher = false
		return
	}
	if v.callbacks.Hide != nil {
		v.callbacks.Hide()
	}
}

func (v *View) countLabel() string {
	return fmt.Sprintf(v.Text("%d 个字符", "%d characters"), v.Count)
}
