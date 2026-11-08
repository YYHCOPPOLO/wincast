// Package uninstall holds the pure candidate rules for removing an installed
// application and the files it left behind. Filesystem and registry facts are
// injected; this package only decides what is safe to show and select.
package uninstall

import (
	"fmt"
	"path/filepath"
	"strings"
)

type Kind string

const (
	KindInstall      Kind = "install"
	KindAppData      Kind = "appdata"
	KindLocalAppData Kind = "local-appdata"
	KindProgramData  Kind = "program-data"
)

// Candidate is one filesystem item the screen can move to the Recycle Bin.
type Candidate struct {
	ID          string
	Name        string
	Path        string
	DisplayPath string
	Kind        Kind
	IsDir       bool
	Removable   bool
	LockReason  string
	Selected    bool
	Error       string
}

// Facts are the caller's filesystem observations for one candidate.
type Facts struct {
	Exists bool
	IsDir  bool
	Locked bool
	Reason string
}

// Roots is the machine context protection is measured against.
type Roots struct {
	Home         string
	OwnExe       string
	OwnRoot      string
	Windows      string
	ProgramFiles []string
	ProgramData  string
	AppData      string
	LocalAppData string
	OtherNames   []string
}

var reservedNames = map[string]bool{
	"appdata": true, "application": true, "application data": true, "app": true,
	"bin": true, "bin32": true, "bin64": true, "client": true,
	"common files": true, "default": true,
	"desktop": true, "documents": true, "downloads": true, "launcher": true, "local": true,
	"locallow": true, "microsoft": true, "music": true, "packages": true,
	"pictures": true, "program files": true, "program files (x86)": true,
	"programdata": true, "programs": true, "public": true, "setup": true,
	"temp": true, "tmp": true, "uninstall": true, "uninstaller": true, "update": true,
	"users": true, "videos": true, "windows": true,
}

func Fold(value string) string {
	return strings.ToLower(strings.TrimSpace(value))
}

// SafeName reports whether a display name can attribute a folder by itself.
// Install locations are strong evidence; this gate is only for name matches.
func SafeName(name string, others []string) bool {
	key := Fold(name)
	if len([]rune(key)) < 3 || reservedNames[key] {
		return false
	}
	for _, other := range others {
		if Fold(other) == key {
			return false
		}
	}
	return true
}

// NormalizePath returns the cleaned absolute path and whether it is usable.
func NormalizePath(path string) (string, bool) {
	if strings.TrimSpace(path) == "" || strings.ContainsRune(path, 0) {
		return "", false
	}
	if strings.HasPrefix(path, `\\`) {
		// UNC share roots and device paths are outside this feature's scope.
		return "", false
	}
	absolute, err := filepath.Abs(path)
	if err != nil {
		return "", false
	}
	cleaned := filepath.Clean(absolute)
	volume := filepath.VolumeName(cleaned)
	if volume == "" || cleaned == volume+string(filepath.Separator) {
		return "", false
	}
	return cleaned, true
}

func IsDescendant(path, ancestor string) bool {
	path, ok := NormalizePath(path)
	if !ok {
		return false
	}
	ancestor, ok = NormalizePath(ancestor)
	if !ok {
		return false
	}
	path, ancestor = Fold(path), Fold(ancestor)
	if path == ancestor {
		return false
	}
	prefix := ancestor
	if !strings.HasSuffix(prefix, string(filepath.Separator)) {
		prefix += string(filepath.Separator)
	}
	return strings.HasPrefix(path, prefix)
}

// Protected explains why a path may not be offered at all. It is applied to
// every candidate regardless of which rule attributed it.
func (r Roots) Protected(path string) (bool, string) {
	cleaned, ok := NormalizePath(path)
	if !ok {
		return true, "invalid path"
	}
	folded := Fold(cleaned)
	equal := func(other string) bool {
		if other == "" {
			return false
		}
		value, valid := NormalizePath(other)
		return valid && Fold(value) == folded
	}
	if equal(r.Home) || equal(r.OwnExe) || equal(r.OwnRoot) || equal(r.Windows) ||
		equal(r.ProgramData) || equal(r.AppData) || equal(r.LocalAppData) {
		return true, "protected location"
	}
	for _, root := range r.ProgramFiles {
		if equal(root) {
			return true, "protected location"
		}
	}
	if r.Windows != "" && IsDescendant(cleaned, r.Windows) {
		return true, "under Windows"
	}
	if r.OwnRoot != "" && (IsDescendant(cleaned, r.OwnRoot) || IsDescendant(r.OwnRoot, cleaned)) {
		return true, "Tinycast installation"
	}
	// A drive root is already rejected by NormalizePath; a one-component path
	// such as C:\Temp is still too broad to claim for an app.
	relative, err := filepath.Rel(filepath.VolumeName(cleaned)+string(filepath.Separator), cleaned)
	if err == nil && !strings.ContainsRune(relative, filepath.Separator) {
		return true, "top-level directory"
	}
	return false, ""
}

// Classify applies existence, protection and lock facts to one path.
func Classify(name, displayPath, path string, kind Kind, facts Facts, roots Roots) (Candidate, bool) {
	cleaned, ok := NormalizePath(path)
	if !ok || !facts.Exists {
		return Candidate{}, false
	}
	if protected, _ := roots.Protected(cleaned); protected {
		return Candidate{}, false
	}
	if displayPath == "" {
		displayPath = cleaned
	}
	candidate := Candidate{
		ID: Fold(cleaned), Name: name, Path: cleaned, DisplayPath: displayPath,
		Kind: kind, IsDir: facts.IsDir, Removable: !facts.Locked,
	}
	if facts.Locked {
		candidate.LockReason = facts.Reason
		if candidate.LockReason == "" {
			candidate.LockReason = "locked"
		}
	} else {
		candidate.Selected = true
	}
	return candidate, true
}

// DedupeNested keeps the outermost candidate when one selected folder already
// contains another, so a single trash operation cannot act twice on one tree.
func DedupeNested(items []Candidate) []Candidate {
	if len(items) < 2 {
		return items
	}
	out := make([]Candidate, 0, len(items))
	for _, item := range items {
		contained := false
		for _, kept := range out {
			if IsDescendant(item.Path, kept.Path) {
				contained = true
				break
			}
		}
		if !contained {
			out = append(out, item)
		}
	}
	return out
}

func Filter(items []Candidate, query string) []Candidate {
	query = Fold(query)
	if query == "" {
		return items
	}
	out := make([]Candidate, 0, len(items))
	for _, item := range items {
		if strings.Contains(Fold(item.Name), query) || strings.Contains(Fold(item.Path), query) {
			out = append(out, item)
		}
	}
	return out
}

func Selected(items []Candidate) []Candidate {
	out := make([]Candidate, 0, len(items))
	for _, item := range items {
		if item.Selected && item.Removable {
			out = append(out, item)
		}
	}
	return out
}

func SelectedCount(items []Candidate) int {
	count := 0
	for _, item := range items {
		if item.Selected && item.Removable {
			count++
		}
	}
	return count
}

// Toggle flips a removable row and leaves a locked one untouched.
func Toggle(items []Candidate, index int) {
	if index < 0 || index >= len(items) || !items[index].Removable {
		return
	}
	items[index].Selected = !items[index].Selected
}

// Abbreviate shortens known machine roots for a row subtitle.
func Abbreviate(path string, roots Roots) string {
	for _, entry := range []struct {
		root string
		env  string
	}{
		{roots.LocalAppData, "%LOCALAPPDATA%"},
		{roots.AppData, "%APPDATA%"},
		{roots.ProgramData, "%PROGRAMDATA%"},
		{roots.Home, "%USERPROFILE%"},
	} {
		if entry.root == "" {
			continue
		}
		if path == entry.root {
			return entry.env
		}
		if IsDescendant(path, entry.root) {
			relative, err := filepath.Rel(entry.root, path)
			if err == nil {
				return fmt.Sprintf("%s\\%s", entry.env, relative)
			}
		}
	}
	return path
}
