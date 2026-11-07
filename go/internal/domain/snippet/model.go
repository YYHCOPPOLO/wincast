// Package snippet holds the Markdown snippet model and its user-authored
// frontmatter format. The source stays literal: nothing is rewritten except
// the canonical frontmatter on save.
package snippet

import (
	"fmt"
	"path/filepath"
	"strings"
	"unicode"
)

// Snippet is one Markdown file. ID is the absolute source path, so an external
// rename appears as a delete plus a create.
type Snippet struct {
	ID               string
	Name             string
	Keyword          string
	Enabled          bool
	ShowConfirmation bool
	Body             string
	Revision         string
}

// EntryID is the launcher identity for a snippet.
func EntryID(id string) string { return "snippet:" + id }

// IDFromEntry returns the snippet path from a launcher identity.
func IDFromEntry(entryID string) (string, bool) {
	return strings.CutPrefix(entryID, "snippet:")
}

// RevisionOf fingerprints source bytes. It is deterministic and content
// sensitive, never a timestamp.
func RevisionOf(content string) string {
	var hash uint64 = 14695981039346656037
	for i := 0; i < len(content); i++ {
		hash ^= uint64(content[i])
		hash *= 1099511628211
	}
	return fmt.Sprintf("%d:%x", len(content), hash)
}

// DefaultName derives a display name from a file name: the stem, dashes
// turned into spaces and each word capitalized.
func DefaultName(path string) string {
	stem := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	stem = strings.ReplaceAll(stem, "-", " ")
	return capitalizeWords(stem)
}

func capitalizeWords(text string) string {
	var builder strings.Builder
	newWord := true
	for _, ch := range text {
		switch {
		case ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r':
			builder.WriteRune(ch)
			newWord = true
		case newWord:
			builder.WriteString(strings.ToUpper(string(ch)))
			newWord = false
		default:
			builder.WriteString(strings.ToLower(string(ch)))
		}
	}
	return builder.String()
}

// Slug is the filename base for a name: lower-case words joined by dashes.
func Slug(name string) string {
	var parts []string
	var current strings.Builder
	flush := func() {
		if current.Len() > 0 {
			parts = append(parts, current.String())
			current.Reset()
		}
	}
	for _, ch := range name {
		if isAlphaNumeric(ch) {
			current.WriteString(strings.ToLower(string(ch)))
			continue
		}
		flush()
	}
	flush()
	if len(parts) == 0 {
		return "snippet"
	}
	return strings.Join(parts, "-")
}

func isAlphaNumeric(ch rune) bool {
	return unicode.IsLetter(ch) || unicode.IsNumber(ch)
}

// ParseError names the file and line that made the frontmatter invalid.
type ParseError struct {
	Path   string
	Line   int
	Reason string
}

func (e *ParseError) Error() string {
	return fmt.Sprintf("%s:%d: %s", e.Path, e.Line, e.Reason)
}

type sourceLine struct {
	text string
	end  int
}

func sourceLines(content string) []sourceLine {
	lines := make([]sourceLine, 0, 8)
	start := 0
	for i := 0; i < len(content); {
		if content[i] != '\n' && content[i] != '\r' {
			i++
			continue
		}
		end := i + 1
		if content[i] == '\r' && end < len(content) && content[end] == '\n' {
			end++
		}
		lines = append(lines, sourceLine{text: content[start:i], end: end})
		start, i = end, end
	}
	if start < len(content) || len(lines) == 0 {
		lines = append(lines, sourceLine{text: content[start:], end: len(content)})
	}
	return lines
}

// ParseMarkdown reads optional frontmatter and keeps the body byte-for-byte.
func ParseMarkdown(path, content string) (Snippet, error) {
	lines := sourceLines(content)
	if len(lines) == 0 || lines[0].text != "---" {
		return Snippet{
			ID: path, Name: DefaultName(path), Enabled: true, Body: content, Revision: RevisionOf(content),
		}, nil
	}
	closing := -1
	for index := 1; index < len(lines); index++ {
		if lines[index].text == "---" {
			closing = index
			break
		}
	}
	if closing < 0 {
		return Snippet{}, &ParseError{Path: path, Line: 1, Reason: "Missing closing frontmatter delimiter"}
	}
	name := ""
	keyword := ""
	enabled := true
	showConfirmation := false
	seen := make(map[string]bool, 4)
	for index := 1; index < closing; index++ {
		line := lines[index]
		if strings.TrimSpace(line.text) == "" {
			continue
		}
		colon := strings.IndexByte(line.text, ':')
		if colon < 0 {
			return Snippet{}, &ParseError{Path: path, Line: index + 1, Reason: "Expected a key and value separated by ':'"}
		}
		rawKey := strings.TrimSpace(line.text[:colon])
		rawValue := strings.TrimSpace(line.text[colon+1:])
		key, ok := canonicalKey(rawKey)
		if !ok {
			return Snippet{}, &ParseError{Path: path, Line: index + 1, Reason: "Unsupported frontmatter key '" + rawKey + "'"}
		}
		if seen[key] {
			return Snippet{}, &ParseError{Path: path, Line: index + 1, Reason: "Duplicate frontmatter key '" + key + "'"}
		}
		seen[key] = true
		switch key {
		case "name":
			decoded, err := decodeScalar(rawValue, path, index+1)
			if err != nil {
				return Snippet{}, err
			}
			if strings.TrimSpace(decoded) != "" {
				name = decoded
			}
		case "keyword":
			decoded, err := decodeScalar(rawValue, path, index+1)
			if err != nil {
				return Snippet{}, err
			}
			keyword = decoded
		case "enabled":
			decoded, err := decodeBoolean(rawValue, path, index+1)
			if err != nil {
				return Snippet{}, err
			}
			enabled = decoded
		case "show_confirmation":
			decoded, err := decodeBoolean(rawValue, path, index+1)
			if err != nil {
				return Snippet{}, err
			}
			showConfirmation = decoded
		}
	}
	if name == "" {
		name = DefaultName(path)
	}
	body := content[lines[closing].end:]
	return Snippet{
		ID: path, Name: name, Keyword: keyword, Enabled: enabled,
		ShowConfirmation: showConfirmation, Body: body, Revision: RevisionOf(content),
	}, nil
}

func canonicalKey(raw string) (string, bool) {
	switch strings.ToLower(raw) {
	case "name":
		return "name", true
	case "keyword":
		return "keyword", true
	case "enabled":
		return "enabled", true
	case "show_confirmation":
		return "show_confirmation", true
	}
	return "", false
}

func decodeScalar(value, path string, line int) (string, error) {
	if !strings.HasPrefix(value, `"`) {
		return "", &ParseError{Path: path, Line: line, Reason: "String values must use double quotes"}
	}
	var builder strings.Builder
	escaped := false
	for index := 1; index < len(value); index++ {
		ch := value[index]
		if escaped {
			switch ch {
			case '\\':
				builder.WriteByte('\\')
			case '"':
				builder.WriteByte('"')
			case 'n':
				builder.WriteByte('\n')
			case 'r':
				builder.WriteByte('\r')
			case 't':
				builder.WriteByte('\t')
			default:
				return "", &ParseError{Path: path, Line: line, Reason: "Unsupported escape sequence"}
			}
			escaped = false
			continue
		}
		switch ch {
		case '\\':
			escaped = true
		case '"':
			for rest := index + 1; rest < len(value); rest++ {
				if value[rest] != ' ' && value[rest] != '\t' {
					return "", &ParseError{Path: path, Line: line, Reason: "Unexpected text after quoted value"}
				}
			}
			return builder.String(), nil
		case '\n', '\r', '\t':
			return "", &ParseError{Path: path, Line: line, Reason: "Control characters must be escaped"}
		default:
			builder.WriteByte(ch)
		}
	}
	return "", &ParseError{Path: path, Line: line, Reason: "Unterminated quoted value"}
}

func decodeBoolean(value, path string, line int) (bool, error) {
	switch value {
	case "true":
		return true, nil
	case "false":
		return false, nil
	}
	return false, &ParseError{Path: path, Line: line, Reason: "Boolean values must be exactly 'true' or 'false'"}
}

func encodeScalar(value string) string {
	var builder strings.Builder
	builder.WriteByte('"')
	for _, ch := range value {
		switch ch {
		case '\\':
			builder.WriteString(`\\`)
		case '"':
			builder.WriteString(`\"`)
		case '\n':
			builder.WriteString(`\n`)
		case '\r':
			builder.WriteString(`\r`)
		case '\t':
			builder.WriteString(`\t`)
		default:
			builder.WriteRune(ch)
		}
	}
	builder.WriteByte('"')
	return builder.String()
}

// Serialize writes canonical frontmatter followed by the literal body.
func Serialize(s Snippet) string {
	var builder strings.Builder
	builder.WriteString("---\n")
	builder.WriteString("name: ")
	builder.WriteString(encodeScalar(s.Name))
	builder.WriteByte('\n')
	if s.Keyword != "" {
		builder.WriteString("keyword: ")
		builder.WriteString(encodeScalar(s.Keyword))
		builder.WriteByte('\n')
	}
	builder.WriteString("enabled: ")
	builder.WriteString(boolText(s.Enabled))
	builder.WriteByte('\n')
	builder.WriteString("show_confirmation: ")
	builder.WriteString(boolText(s.ShowConfirmation))
	builder.WriteString("\n---\n")
	builder.WriteString(s.Body)
	return builder.String()
}

func boolText(value bool) string {
	if value {
		return "true"
	}
	return "false"
}
