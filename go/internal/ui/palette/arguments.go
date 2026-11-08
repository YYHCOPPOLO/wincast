package palette

import (
	"fmt"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/template"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
)

// ArgumentForm is one captured argument prompt. Answers follow the spec order
// and are committed one field at a time because the palette has one input.
type ArgumentForm struct {
	Entry   domain.Application
	Specs   []template.Argument
	Values  []string
	Index   int
	Choice  int
	Problem string
}

// SetModeArguments starts collecting the arguments a template still needs.
func (v *View) SetModeArguments(entry domain.Application, specs []template.Argument) bool {
	if len(specs) == 0 {
		return false
	}
	if v.Mode == ModeLauncher {
		v.launcherQuery = v.Query
	}
	v.Mode = ModeArguments
	v.Query = ""
	v.inputText.Set("")
	v.Argument = &ArgumentForm{
		Entry:  entry,
		Specs:  append([]template.Argument(nil), specs...),
		Values: make([]string, len(specs)),
	}
	v.argumentList.Selected = nil
	v.argumentList.ScrollIntoView(0)
	v.Input.Focus()
	return true
}

func (v *View) InArguments() bool { return v.Mode == ModeArguments }

// ArgumentValues returns the answers once every field has been committed.
func (v *View) ArgumentValues() []string {
	if v.Argument == nil {
		return nil
	}
	return append([]string(nil), v.Argument.Values...)
}

func (v *View) cancelArguments() {
	v.Argument = nil
	v.launcherQuery = ""
	v.SetModeLauncher()
}

func (v *View) argumentCurrent() *template.Argument {
	if v.Argument == nil || v.Argument.Index < 0 || v.Argument.Index >= len(v.Argument.Specs) {
		return nil
	}
	return &v.Argument.Specs[v.Argument.Index]
}

func (v *View) argumentCommit() {
	form := v.Argument
	if form == nil {
		return
	}
	value := v.Query
	if options := form.Specs[form.Index].Options; len(options) > 0 {
		choice := form.Choice
		if choice < 0 || choice >= len(options) {
			choice = 0
		}
		value = options[choice]
	}
	form.Values[form.Index] = value
	if form.Index+1 < len(form.Specs) {
		form.Index++
		form.Choice = 0
		form.Problem = ""
		v.Query = ""
		v.inputText.Set("")
		v.argumentList.ScrollIntoView(form.Index)
		return
	}
	entry := form.Entry
	values := append([]string(nil), form.Values...)
	v.Argument = nil
	v.Query = ""
	v.inputText.Set("")
	v.launcherQuery = ""
	v.Mode = ModeLauncher
	v.Filter()
	if v.callbacks.SubmitArguments != nil {
		v.callbacks.SubmitArguments(entry, values)
	}
	v.Input.Focus()
}

func (v *View) argumentStepBack() {
	form := v.Argument
	if form == nil || form.Index == 0 {
		return
	}
	form.Index--
	form.Choice = 0
	v.Query = form.Values[form.Index]
	v.inputText.Set(v.Query)
	v.argumentList.ScrollIntoView(form.Index)
}

func (v *View) argumentMoveChoice(step int) {
	form := v.Argument
	if form == nil {
		return
	}
	options := form.Specs[form.Index].Options
	if len(options) == 0 {
		return
	}
	form.Choice = max(0, min(form.Choice+step, len(options)-1))
}

func (v *View) argumentChoose(option int) {
	form := v.Argument
	if form == nil {
		return
	}
	options := form.Specs[form.Index].Options
	if option < 0 || option >= len(options) {
		return
	}
	form.Choice = option
	v.Query = options[option]
	v.inputText.Set(v.Query)
}

func (v *View) argumentsBody(c *ui.Context, tokens theme.Tokens) {
	form := v.Argument
	if form == nil {
		return
	}
	current := form.Specs[form.Index]
	progress := fmt.Sprintf(v.Text("参数 %d/%d", "Argument %d/%d"), form.Index+1, len(form.Specs))
	ui.Row(c).Padding(8, 18).Gap(10).AlignItems(ui.Center).Children(func() {
		ui.Text(c, form.Entry.Name+" · "+progress).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Grow(1).SingleLine().Role(ui.RoleHeading)
		ui.Text(c, current.Name).FontSize(theme.RowFont).TextColor(tokens.Text).SingleLine()
	})
	ui.Scroll(c).Grow(1).Children(func() {
		ui.Column(c).FillWidth().Padding(6, 18).Gap(2).Children(func() {
			for index, spec := range form.Specs {
				subtitle := form.Values[index]
				if subtitle == "" && index > form.Index {
					subtitle = v.Text("尚未填写", "Not set yet")
				}
				components.ResultRow(c, tokens, components.Row{
					Fallback: v.argumentBadge(index),
					Title:    spec.Name,
					Subtitle: subtitle,
					Selected: index == form.Index,
					Trailing: specOptionsBadge(len(spec.Options)),
				})
			}
			if len(current.Options) > 0 {
				components.Section(c, tokens, v.Text("选项", "Options"))
				for index, option := range current.Options {
					selected := index == form.Choice
					ui.Text(c, option).FontSize(theme.RowFont).
						Height(theme.RowHeight).Padding(6, 12).Radius(theme.RowRadius).
						Background(selectionFill(tokens, selected)).
						TextColor(tokens.Text).SingleLine().Role(ui.RoleButton).
						OnClick(func() { v.argumentChoose(index) })
				}
			}
			if form.Problem != "" {
				ui.Text(c, form.Problem).FontSize(theme.TrailingFont).TextColor(tokens.Destructive).Padding(6, 12)
			}
		})
	})
}

func selectionFill(tokens theme.Tokens, selected bool) ui.Color {
	if selected {
		return tokens.Selection
	}
	return ui.Transparent
}

func (v *View) argumentBadge(index int) string {
	if v.Argument == nil {
		return ""
	}
	if index < v.Argument.Index {
		return "✓"
	}
	if index == v.Argument.Index {
		return "›"
	}
	return string(rune('1' + index%9))
}

func specOptionsBadge(count int) string {
	if count == 0 {
		return ""
	}
	return "+" + string(rune('0'+min(count, 9)))
}
