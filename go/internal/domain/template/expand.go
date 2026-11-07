package template

import (
	"strings"
	"time"
)

// Context is one captured expansion environment. Callers capture it once,
// before an argument prompt, so clipboard, selection and clock cannot drift
// while the form is open.
type Context struct {
	Clipboard []string
	Selection string
	Now       time.Time
	Locale    string
	NewUUID   func() string
}

// Argument is one unresolved template argument. Options is never empty for a
// picker prompt.
type Argument struct {
	Name    string
	Options []string
}

// Output is the expanded text plus the caret offset in graphemes.
type Output struct {
	Text      string
	Cursor    int
	HasCursor bool
	Arguments []Argument
}

// Snippet is the part of a snippet record the engine can nest.
type Snippet struct {
	ID      string
	Name    string
	Keyword string
	Enabled bool
	Body    string
}

const maxReferenceDepth = 5

// Expand resolves every value token and leaves unresolved arguments visible.
func Expand(input string, ctx Context, args map[string]string) Output {
	return expand(input, ctx, args, nil, false, 0, nil, false)
}

// ExpandWith resolves a template and percent-encodes substituted values when
// autoPercent is set, unless the token opted out with raw or already encoded.
func ExpandWith(input string, ctx Context, args map[string]string, autoPercent bool) Output {
	return expand(input, ctx, args, nil, autoPercent, 0, nil, false)
}

// ExpandDestination keeps cursor and snippet references literal.
func ExpandDestination(input string, ctx Context, args map[string]string, autoPercent bool) Output {
	return expand(input, ctx, args, nil, autoPercent, 0, nil, true)
}

// ExpandSnippet nests snippets by name or keyword.
func ExpandSnippet(input string, ctx Context, args map[string]string, snippets []Snippet, rootID string) Output {
	visited := make(map[string]bool, 1)
	if rootID != "" {
		visited[rootID] = true
	}
	return expand(input, ctx, args, snippets, false, 0, visited, false)
}

// UsesSelection reports whether a template reads the selected text.
func UsesSelection(input string) bool {
	for _, part := range parseSegments(input) {
		if part.Kind == segmentSelection {
			return true
		}
	}
	return false
}

func expand(input string, ctx Context, args map[string]string, snippets []Snippet, autoPercent bool, depth int, visited map[string]bool, destination bool) Output {
	var text strings.Builder
	cursor := 0
	hasCursor := false
	arguments := make([]Argument, 0, 4)
	names := make(map[string]bool, 4)
	sorted := nilIfEmpty(snippets)

	for _, part := range parseSegments(input) {
		switch part.Kind {
		case segmentLiteral:
			text.WriteString(part.Literal)
		case segmentClipboard:
			value := ""
			if part.Offset >= 0 && part.Offset < len(ctx.Clipboard) {
				value = ctx.Clipboard[part.Offset]
			}
			text.WriteString(applyModifiers(part.Modifiers, value, autoPercent))
		case segmentSelection:
			text.WriteString(applyModifiers(part.Modifiers, ctx.Selection, autoPercent))
		case segmentDateTime:
			text.WriteString(applyModifiers(part.Modifiers, formatDateTime(part.Date, ctx), autoPercent))
		case segmentUUID:
			value := ""
			if ctx.NewUUID != nil {
				value = ctx.NewUUID()
			}
			text.WriteString(applyModifiers(part.Modifiers, value, autoPercent))
		case segmentArgument:
			value, ok := args[part.Argument.Name]
			if !ok && part.Argument.HasDefault {
				value, ok = part.Argument.Default, true
			}
			if ok {
				text.WriteString(applyModifiers(part.Modifiers, value, autoPercent))
				continue
			}
			text.WriteString(part.Source)
			if !names[part.Argument.Name] {
				names[part.Argument.Name] = true
				arguments = append(arguments, Argument{Name: part.Argument.Name, Options: append([]string(nil), part.Argument.Options...)})
			}
		case segmentCursor:
			if destination {
				text.WriteString("{cursor}")
			} else if !hasCursor {
				hasCursor, cursor = true, graphemeCount(text.String())
			}
		case segmentSnippetRef:
			if destination {
				text.WriteString(part.Source)
				continue
			}
			target, found := resolveReference(part.Key, sorted)
			if !found || depth >= maxReferenceDepth || visited[target.ID] {
				text.WriteString(part.Source)
				continue
			}
			nestedVisited := make(map[string]bool, len(visited)+1)
			for id := range visited {
				nestedVisited[id] = true
			}
			nestedVisited[target.ID] = true
			nested := expand(target.Body, ctx, args, snippets, autoPercent, depth+1, nestedVisited, false)
			insertion := graphemeCount(text.String())
			if !hasCursor && nested.HasCursor {
				hasCursor, cursor = true, insertion+nested.Cursor
			}
			text.WriteString(nested.Text)
			for _, argument := range nested.Arguments {
				if !names[argument.Name] {
					names[argument.Name] = true
					arguments = append(arguments, argument)
				}
			}
		}
	}

	return Output{Text: text.String(), Cursor: cursor, HasCursor: hasCursor, Arguments: arguments}
}

func nilIfEmpty(snippets []Snippet) []Snippet {
	if len(snippets) == 0 {
		return nil
	}
	// Resolution order is file-identity order, never catalog order.
	sorted := append([]Snippet(nil), snippets...)
	for i := 1; i < len(sorted); i++ {
		for j := i; j > 0 && sorted[j].ID < sorted[j-1].ID; j-- {
			sorted[j], sorted[j-1] = sorted[j-1], sorted[j]
		}
	}
	return sorted
}

func resolveReference(key string, snippets []Snippet) (Snippet, bool) {
	normalized := strings.ToLower(key)
	enabled := make([]Snippet, 0, len(snippets))
	for _, snippet := range snippets {
		if snippet.Enabled {
			enabled = append(enabled, snippet)
		}
	}
	for _, snippet := range enabled {
		if strings.ToLower(snippet.Name) == normalized {
			return snippet, true
		}
	}
	for _, snippet := range enabled {
		if snippet.Keyword != "" && strings.ToLower(snippet.Keyword) == normalized {
			return snippet, true
		}
	}
	return Snippet{}, false
}

func applyModifiers(modifiers []Modifier, value string, autoPercent bool) string {
	modified := value
	optedOut := false
	for _, modifier := range modifiers {
		switch modifier {
		case ModifierUppercase:
			modified = strings.ToUpper(modified)
		case ModifierLowercase:
			modified = strings.ToLower(modified)
		case ModifierTrim:
			modified = strings.TrimSpace(modified)
		case ModifierPercentEncode:
			modified = percentEncode(modified)
			optedOut = true
		case ModifierJSONStringify:
			modified = jsonEscape(modified)
		case ModifierRaw:
			optedOut = true
		}
	}
	if autoPercent && !optedOut {
		return percentEncode(modified)
	}
	return modified
}

func percentEncode(value string) string {
	const hex = "0123456789ABCDEF"
	var builder strings.Builder
	builder.Grow(len(value) * 3 / 2)
	for i := 0; i < len(value); i++ {
		ch := value[i]
		if (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') ||
			ch == '-' || ch == '.' || ch == '_' || ch == '~' {
			builder.WriteByte(ch)
			continue
		}
		builder.WriteByte('%')
		builder.WriteByte(hex[ch>>4])
		builder.WriteByte(hex[ch&0x0f])
	}
	return builder.String()
}

func jsonEscape(value string) string {
	var builder strings.Builder
	for _, ch := range value {
		switch ch {
		case '"':
			builder.WriteString(`\"`)
		case '\\':
			builder.WriteString(`\\`)
		case '\n':
			builder.WriteString(`\n`)
		case '\r':
			builder.WriteString(`\r`)
		case '\t':
			builder.WriteString(`\t`)
		default:
			if ch < 0x20 {
				const hex = "0123456789abcdef"
				builder.WriteString(`\u`)
				builder.WriteByte(hex[(ch>>12)&0x0f])
				builder.WriteByte(hex[(ch>>8)&0x0f])
				builder.WriteByte(hex[(ch>>4)&0x0f])
				builder.WriteByte(hex[ch&0x0f])
			} else {
				builder.WriteRune(ch)
			}
		}
	}
	return builder.String()
}
