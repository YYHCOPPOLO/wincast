// Package domain holds pure decisions: identity, ranking and mode rules.
// It must not import MyGo, Win32, storage or anything that reads a disk.
package domain

import (
	"cmp"
	"slices"
	"strings"
)

// Application is the catalog entry the UI shows. ID is the stable business
// identity (AppUserModelID); Path locates the icon and launch target. Neither
// is derived from the current sort position. Kind is empty for applications
// and names the feature for command, quicklink and snippet entries.
type Application struct {
	ID   string
	Name string
	Path string
	Kind Kind
	// AlternateNames are extra search names that rank below the display name,
	// like the original AppEntry.alternateNames: today the other UI language's
	// title for a built-in command.
	AlternateNames []string
}

// Kind distinguishes catalog entries that are not installed applications.
type Kind string

const (
	KindApplication    Kind = ""
	KindCommand        Kind = "command"
	KindQuicklink      Kind = "quicklink"
	KindSnippet        Kind = "snippet"
	KindSystemAction   Kind = "system-action"
	KindSystemSettings Kind = "system-settings"
	KindWindowCommand  Kind = "window-command"
	KindQuickAction    Kind = "quick-action"
)

// Mode is the palette state that selects both the query source and the rows.
type Mode uint8

const (
	ModeApplications Mode = iota
	ModeDictionary
)

func (m Mode) String() string {
	if m == ModeDictionary {
		return "dictionary"
	}
	return "applications"
}

// Rank scores name against an already lower-cased query. It preserves the
// launcher's current ordering: prefix, word prefix, substring, subsequence.
// A negative result means the application is not a match.
func Rank(name, query string) int {
	if query == "" || strings.HasPrefix(name, query) {
		return 0
	}
	if strings.Contains(name, " "+query) {
		return 1
	}
	if strings.Contains(name, query) {
		return 2
	}
	rest := name
	for _, r := range query {
		i := strings.IndexRune(rest, r)
		if i < 0 {
			return -1
		}
		rest = rest[i+len(string(r)):]
	}
	return 3
}

// Filter applies Rank to the catalog and returns a new slice in stable input
// order per score. It allocates only the result, so it is safe to call for a
// single frame without changing the catalog itself.
func Filter(apps []Application, query string) []Application {
	query = strings.ToLower(strings.TrimSpace(query))
	type ranked struct {
		application Application
		score       int
	}
	items := make([]ranked, 0, len(apps))
	for _, app := range apps {
		if score := Rank(strings.ToLower(app.Name), query); score >= 0 {
			items = append(items, ranked{application: app, score: score})
		}
	}
	// Stable ordering keeps catalog order inside one score band.
	slices.SortStableFunc(items, func(a, b ranked) int { return cmp.Compare(a.score, b.score) })
	results := make([]Application, len(items))
	for i, item := range items {
		results[i] = item.application
	}
	return results
}
