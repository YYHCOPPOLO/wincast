// Package template is the one template engine shared by snippets, quicklinks
// and custom commands. Clock, locale, clipboard and UUID are injected so the
// package stays pure.
package template

import (
	"strconv"
	"strings"
)

type Modifier uint8

const (
	ModifierUppercase Modifier = iota
	ModifierLowercase
	ModifierTrim
	ModifierPercentEncode
	ModifierJSONStringify
	ModifierRaw
)

func parseModifier(raw string) (Modifier, bool) {
	switch raw {
	case "uppercase":
		return ModifierUppercase, true
	case "lowercase":
		return ModifierLowercase, true
	case "trim":
		return ModifierTrim, true
	case "percent-encode":
		return ModifierPercentEncode, true
	case "json-stringify":
		return ModifierJSONStringify, true
	case "raw":
		return ModifierRaw, true
	}
	return 0, false
}

type DateTimeKind uint8

const (
	DateTimeDate DateTimeKind = iota
	DateTimeTime
	DateTimeBoth
	DateTimeWeekday
)

type OffsetUnit uint8

const (
	UnitMinute OffsetUnit = iota
	UnitHour
	UnitDay
	UnitMonth
	UnitYear
)

type DateOffset struct {
	Unit  OffsetUnit
	Value int
}

type DateTimeToken struct {
	Kind    DateTimeKind
	Offsets []DateOffset
	Format  string
	Locale  string
}

type ArgumentToken struct {
	Name       string
	Options    []string
	Default    string
	HasDefault bool
}

type segmentKind uint8

const (
	segmentLiteral segmentKind = iota
	segmentClipboard
	segmentSelection
	segmentDateTime
	segmentUUID
	segmentArgument
	segmentCursor
	segmentSnippetRef
)

type segment struct {
	Kind      segmentKind
	Literal   string
	Offset    int
	Modifiers []Modifier
	Date      DateTimeToken
	Argument  ArgumentToken
	Source    string
	Key       string
}

// parseSegments splits template source into literal and token segments.
// Unknown tokens are left literal exactly as written; braces that do not
// start a token never hide an inner token from a later parse step.
func parseSegments(source string) []segment {
	segments := make([]segment, 0, 8)
	i := 0
	for i < len(source) {
		rel := strings.IndexByte(source[i:], '{')
		if rel < 0 {
			segments = append(segments, segment{Kind: segmentLiteral, Literal: source[i:]})
			break
		}
		if rel > 0 {
			segments = append(segments, segment{Kind: segmentLiteral, Literal: source[i : i+rel]})
		}
		open := i + rel
		afterOpen := open + 1
		relClose := strings.IndexByte(source[afterOpen:], '}')
		if relClose < 0 {
			segments = append(segments, segment{Kind: segmentLiteral, Literal: source[open:]})
			break
		}
		closeAt := afterOpen + relClose
		end := closeAt + 1
		rawToken := source[open:end]
		body := source[afterOpen:closeAt]
		if parsed, ok := segmentFor(body, rawToken); ok {
			segments = append(segments, parsed)
			i = end
		} else {
			segments = append(segments, segment{Kind: segmentLiteral, Literal: "{"})
			i = afterOpen
		}
	}
	return mergeLiterals(segments)
}

func mergeLiterals(segments []segment) []segment {
	out := make([]segment, 0, len(segments))
	for _, current := range segments {
		if current.Kind == segmentLiteral && len(out) > 0 && out[len(out)-1].Kind == segmentLiteral {
			out[len(out)-1].Literal += current.Literal
			continue
		}
		out = append(out, current)
	}
	return out
}

func segmentFor(body, source string) (segment, bool) {
	parts, ok := splitOnPipes(body)
	if !ok || len(parts) == 0 {
		return segment{}, false
	}
	head := parts[0]
	modifiers := make([]Modifier, 0, len(parts)-1)
	for _, raw := range parts[1:] {
		modifier, ok := parseModifier(raw)
		if !ok {
			return segment{}, false
		}
		modifiers = append(modifiers, modifier)
	}

	if head == "cursor" {
		if len(modifiers) == 0 {
			return segment{Kind: segmentCursor}, true
		}
		return segment{}, false
	}
	if key, found := strings.CutPrefix(head, "snippet:"); found {
		key = strings.TrimSpace(key)
		if key == "" || len(modifiers) != 0 {
			return segment{}, false
		}
		return segment{Kind: segmentSnippetRef, Key: key, Source: source}, true
	}

	token, ok := parseToken(head)
	if !ok {
		return segment{}, false
	}
	switch token.Command {
	case "clipboard":
		offset, ok := intParameter(token, "offset", 0)
		if !ok || offset < 0 || !token.hasOnly("offset") {
			return segment{}, false
		}
		return segment{Kind: segmentClipboard, Offset: offset, Modifiers: modifiers}, true
	case "selection", "selectedtext":
		if len(token.Parameters) != 0 {
			return segment{}, false
		}
		return segment{Kind: segmentSelection, Modifiers: modifiers}, true
	case "uuid":
		if len(token.Parameters) != 0 {
			return segment{}, false
		}
		return segment{Kind: segmentUUID, Modifiers: modifiers}, true
	case "date", "time", "datetime", "day":
		date, ok := parseDateTime(token)
		if !ok {
			return segment{}, false
		}
		return segment{Kind: segmentDateTime, Date: date, Modifiers: modifiers}, true
	case "argument":
		argument, ok := parseArgument(token)
		if !ok {
			return segment{}, false
		}
		return segment{Kind: segmentArgument, Argument: argument, Source: source, Modifiers: modifiers}, true
	case "snippet":
		name, ok := token.Parameters["name"]
		if !ok {
			return segment{}, false
		}
		name = strings.TrimSpace(name)
		if name == "" || !token.hasOnly("name") || len(modifiers) != 0 {
			return segment{}, false
		}
		return segment{Kind: segmentSnippetRef, Key: name, Source: source}, true
	}
	return segment{}, false
}

type parsedToken struct {
	Command    string
	Parameters map[string]string
}

func (t parsedToken) hasOnly(allowed ...string) bool {
	for key := range t.Parameters {
		found := false
		for _, candidate := range allowed {
			if key == candidate {
				found = true
				break
			}
		}
		if !found {
			return false
		}
	}
	return true
}

func intParameter(token parsedToken, key string, fallback int) (int, bool) {
	raw, ok := token.Parameters[key]
	if !ok {
		return fallback, true
	}
	value, err := strconv.Atoi(raw)
	if err != nil {
		return 0, false
	}
	return value, true
}

func parseDateTime(token parsedToken) (DateTimeToken, bool) {
	if !token.hasOnly("offset", "format", "locale") {
		return DateTimeToken{}, false
	}
	format, hasFormat := token.Parameters["format"]
	locale, hasLocale := token.Parameters["locale"]
	if hasFormat && hasLocale {
		return DateTimeToken{}, false
	}
	if hasFormat && format == "" {
		return DateTimeToken{}, false
	}
	if hasLocale && locale == "" {
		return DateTimeToken{}, false
	}
	var offsets []DateOffset
	if raw, ok := token.Parameters["offset"]; ok {
		parsed, ok := parseOffsets(raw)
		if !ok {
			return DateTimeToken{}, false
		}
		offsets = parsed
	}
	kind := DateTimeWeekday
	switch token.Command {
	case "date":
		kind = DateTimeDate
	case "time":
		kind = DateTimeTime
	case "datetime":
		kind = DateTimeBoth
	}
	return DateTimeToken{Kind: kind, Offsets: offsets, Format: format, Locale: locale}, true
}

func parseOffsets(raw string) ([]DateOffset, bool) {
	pieces := strings.Fields(raw)
	if len(pieces) == 0 {
		return nil, false
	}
	offsets := make([]DateOffset, 0, len(pieces))
	for _, piece := range pieces {
		if piece == "" {
			return nil, false
		}
		unitChar := piece[len(piece)-1]
		var unit OffsetUnit
		switch unitChar {
		case 'm':
			unit = UnitMinute
		case 'h':
			unit = UnitHour
		case 'd':
			unit = UnitDay
		case 'M':
			unit = UnitMonth
		case 'y':
			unit = UnitYear
		default:
			return nil, false
		}
		value, err := strconv.Atoi(piece[:len(piece)-1])
		if err != nil {
			return nil, false
		}
		offsets = append(offsets, DateOffset{Unit: unit, Value: value})
	}
	return offsets, true
}

func parseArgument(token parsedToken) (ArgumentToken, bool) {
	if !token.hasOnly("name", "default", "options") {
		return ArgumentToken{}, false
	}
	name := "Argument"
	if raw, ok := token.Parameters["name"]; ok {
		if trimmed := strings.TrimSpace(raw); trimmed != "" {
			name = trimmed
		}
	}
	var options []string
	if raw, ok := token.Parameters["options"]; ok {
		for _, option := range strings.Split(raw, ",") {
			option = strings.TrimSpace(option)
			if option != "" {
				options = append(options, option)
			}
		}
		if len(options) == 0 {
			return ArgumentToken{}, false
		}
	}
	argument := ArgumentToken{Name: name, Options: options}
	if value, ok := token.Parameters["default"]; ok {
		argument.Default, argument.HasDefault = value, true
	}
	return argument, true
}

func splitOnPipes(body string) ([]string, bool) {
	parts := make([]string, 0, 2)
	var current strings.Builder
	inQuotes := false
	escaped := false
	for _, ch := range body {
		if escaped {
			current.WriteRune(ch)
			escaped = false
			continue
		}
		switch {
		case ch == '\\' && inQuotes:
			current.WriteRune(ch)
			escaped = true
		case ch == '"':
			inQuotes = !inQuotes
			current.WriteRune(ch)
		case ch == '|' && !inQuotes:
			parts = append(parts, strings.TrimSpace(current.String()))
			current.Reset()
		default:
			current.WriteRune(ch)
		}
	}
	if inQuotes || escaped {
		return nil, false
	}
	parts = append(parts, strings.TrimSpace(current.String()))
	for _, part := range parts {
		if part == "" {
			return nil, false
		}
	}
	return parts, true
}

func parseToken(head string) (parsedToken, bool) {
	remainder := strings.TrimLeft(head, " \t\r\n")
	commandEnd := strings.IndexAny(remainder, " \t\r\n=")
	if commandEnd < 0 {
		commandEnd = len(remainder)
	}
	if commandEnd == 0 {
		return parsedToken{}, false
	}
	token := parsedToken{
		Command:    strings.ToLower(remainder[:commandEnd]),
		Parameters: make(map[string]string, 4),
	}
	remainder = strings.TrimLeft(remainder[commandEnd:], " \t\r\n")
	for remainder != "" {
		remainder = strings.TrimLeft(remainder, " \t\r\n")
		if remainder == "" {
			break
		}
		keyEnd := strings.IndexAny(remainder, " \t\r\n=")
		if keyEnd < 0 {
			keyEnd = len(remainder)
		}
		if keyEnd == 0 {
			return parsedToken{}, false
		}
		key := remainder[:keyEnd]
		remainder = strings.TrimLeft(remainder[keyEnd:], " \t\r\n")
		if !strings.HasPrefix(remainder, "=") {
			return parsedToken{}, false
		}
		remainder = strings.TrimLeft(remainder[1:], " \t\r\n")
		var value string
		if strings.HasPrefix(remainder, "\"") {
			decoded, rest, ok := decodeQuoted(remainder[1:])
			if !ok {
				return parsedToken{}, false
			}
			value, remainder = decoded, rest
		} else {
			valueEnd := strings.IndexAny(remainder, " \t\r\n")
			if valueEnd < 0 {
				valueEnd = len(remainder)
			}
			if valueEnd == 0 {
				return parsedToken{}, false
			}
			value, remainder = remainder[:valueEnd], remainder[valueEnd:]
		}
		if _, exists := token.Parameters[key]; exists {
			return parsedToken{}, false
		}
		token.Parameters[key] = value
	}
	return token, true
}

func decodeQuoted(input string) (decoded, rest string, ok bool) {
	var builder strings.Builder
	escaped := false
	for i := 0; i < len(input); {
		ch := input[i]
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
				return "", "", false
			}
			escaped = false
			i++
			continue
		}
		switch ch {
		case '\\':
			escaped = true
			i++
		case '"':
			return builder.String(), input[i+1:], true
		default:
			builder.WriteByte(ch)
			i++
		}
	}
	return "", "", false
}
