package palette

import (
	"fmt"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
)

type Definition struct {
	Word, Phonetic string
	Badges         []string
	Sections       []DefinitionSection
}

type DefinitionSection struct{ Label, Text string }

func (v *View) dictionaryBody(c *ui.Context, tokens theme.Tokens) {
	ui.Scroll(c).Grow(1).TrackScroll(&v.dictionaryScroll).Children(func() {
		ui.Column(c).FillWidth().Padding(16, 20).Gap(14).Selectable().Children(func() {
			if v.Definition == nil {
				message := v.Text("输入英文单词查询词典", "Type an English word to look it up")
				if strings.TrimSpace(v.Query) != "" {
					message = v.Text("没有找到词条", "No dictionary entry found")
				}
				if v.DictionaryLoading {
					message = v.Text("正在查询…", "Looking up…")
				}
				if v.DictionaryError != "" {
					message = v.DictionaryError
				}
				ui.Text(c, message).TextColor(tokens.TextTertiary).Role(ui.RoleStatus)
				if len(v.Suggestions) > 0 {
					ui.Text(c, v.Text("你是不是想找：", "Did you mean:")).FontSize(theme.SectionFont).
						TextColor(tokens.TextTertiary)
					ui.Column(c).Gap(4).Children(func() {
						for _, suggestion := range v.Suggestions {
							if ui.Button(c, suggestion).Clicked() {
								v.SetQuery(suggestion)
							}
						}
					})
				}
				return
			}
			ui.Text(c, v.Definition.Word).FontSize(theme.HeadingFont).Bold().Role(ui.RoleHeading)
			if v.Definition.Phonetic != "" {
				ui.Text(c, v.Definition.Phonetic).TextColor(tokens.TextTertiary)
			}
			if len(v.Definition.Badges) > 0 {
				ui.Text(c, strings.Join(v.Definition.Badges, " · ")).FontSize(theme.TrailingFont).
					TextColor(tokens.TextTertiary).Selectable()
			}
			for _, section := range v.Definition.Sections {
				if section.Text == "" {
					continue
				}
				ui.Column(c).Gap(6).Children(func() {
					ui.Text(c, section.Label).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
					ui.Text(c, section.Text)
				})
			}
		})
	})
}

func (v *View) CopyText() string {
	// A native menu can run before Changed has rebuilt the view. Invalidate
	// an old definition before deciding what the current input can copy.
	v.syncQuery(v.inputText.String())
	if v.Mode != ModeDictionary || v.Definition == nil {
		return v.Query
	}
	var text strings.Builder
	text.WriteString(v.Definition.Word)
	if v.Definition.Phonetic != "" {
		fmt.Fprintf(&text, "  %s", v.Definition.Phonetic)
	}
	if len(v.Definition.Badges) > 0 {
		fmt.Fprintf(&text, "\n%s", strings.Join(v.Definition.Badges, " · "))
	}
	for _, section := range v.Definition.Sections {
		if section.Text != "" {
			fmt.Fprintf(&text, "\n\n%s\n%s", section.Label, section.Text)
		}
	}
	return text.String()
}
