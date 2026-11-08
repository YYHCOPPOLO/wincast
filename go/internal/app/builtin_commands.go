package app

import (
	"slices"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
)

// builtinCommand is one Tinycast-owned launcher row, mirroring the original
// CommandCatalog: a stable "command:" identity, a localized title, and the
// other language's title as an alternate search name. They are ordinary
// catalog entries, so the same relevance ranking can place them above apps for
// a query such as "clipboard"; no special priority is applied.
type builtinCommand struct {
	id        string
	zh        string
	en        string
	alternate []string
	visible   func(*controller) bool
}

// builtinCommands keeps the original CommandID order for the command slice.
// A nil visible means the command is always available.
var builtinCommands = []builtinCommand{
	{
		id: "command:ai-chat", zh: "AI 对话", en: "AI Chat",
		visible: func(c *controller) bool { return c.aiEnabled },
	},
	{
		id: "command:calculator-history", zh: "计算器历史", en: "Calculator History",
	},
	{
		id: "command:clipboard-history", zh: "剪贴板历史", en: "Clipboard History",
		alternate: []string{"剪切板"},
		visible:   func(c *controller) bool { return c.clipboardEnabled.Load() },
	},
	{
		id: "command:search-emoji", zh: "搜索表情与符号", en: "Search Emoji & Symbols",
	},
	{
		id: "command:search-files", zh: "搜索文件", en: "Search Files",
		visible: func(c *controller) bool { return c.featureFileSearch },
	},
	{
		id: "command:my-schedule", zh: "我的日程", en: "My Schedule",
		visible: func(c *controller) bool { return c.featureCalendar && c.calendarShowInLauncher },
	},
	{
		id: "command:show-notes", zh: "显示笔记", en: "Show Notes",
		visible: func(c *controller) bool { return c.featureNotes },
	},
	{
		id: "command:settings", zh: "设置", en: "Settings",
	},
}

// builtinEntries builds the visible command slice. The other UI language's
// title is searchable too, so a Chinese window still answers "clipboard" and
// an English window still answers "剪贴板".
func (c *controller) builtinEntries() []domain.Application {
	entries := make([]domain.Application, 0, len(builtinCommands))
	for _, command := range builtinCommands {
		if command.visible != nil && !command.visible(c) {
			continue
		}
		name, other := command.en, command.zh
		if c.language == "zh" {
			name, other = command.zh, command.en
		}
		alternate := make([]string, 0, 1+len(command.alternate))
		alternate = append(alternate, other)
		alternate = append(alternate, command.alternate...)
		entries = append(entries, domain.Application{
			ID: command.id, Name: name, Kind: domain.KindCommand, AlternateNames: alternate,
		})
	}
	// CommandCatalog.all is name-sorted, and the empty query lists the command
	// slice in that order, so do the same here.
	slices.SortStableFunc(entries, func(left, right domain.Application) int {
		return strings.Compare(strings.ToLower(left.Name), strings.ToLower(right.Name))
	})
	return entries
}

// runBuiltinCommand dispatches a built-in row before the custom command store
// sees the id. It reports false for every user-authored command.
func (c *controller) runBuiltinCommand(id string) bool {
	switch id {
	case "command:ai-chat":
		c.openAIChat()
	case "command:calculator-history":
		c.openCalculatorHistory()
	case "command:clipboard-history":
		c.openClipboard()
	case "command:search-emoji":
		c.openEmoji()
	case "command:search-files":
		c.openFileSearch()
	case "command:my-schedule":
		c.openSchedule()
	case "command:show-notes":
		c.openEditor()
	case "command:settings":
		c.openSettings()
	default:
		return false
	}
	return true
}
