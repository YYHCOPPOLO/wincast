// Package backup writes and reads Tinycast's portable backup archive: one ZIP
// holding a manifest, filtered settings, clipboard history, snippets, notes and
// launcher learning. AI keys, chat history and caches never travel.
package backup

import (
	"archive/zip"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"path"
	"sort"
	"strings"
	"time"
)

// Format is the only archive version this build writes or reads.
const Format = 1

// Category identifiers, also used as manifest count keys.
const (
	CategorySettings  = "settings"
	CategoryClipboard = "clipboard"
	CategorySnippets  = "snippets"
	CategoryNotes     = "notes"
	CategoryLearning  = "learning"
)

// Selection says which categories a bundle carries.
type Selection struct {
	Settings  bool
	Clipboard bool
	Snippets  bool
	Notes     bool
	Learning  bool
}

// All selects every category.
func All() Selection {
	return Selection{Settings: true, Clipboard: true, Snippets: true, Notes: true, Learning: true}
}

// Categories lists the selected category keys.
func (s Selection) Categories() []string {
	var categories []string
	if s.Settings {
		categories = append(categories, CategorySettings)
	}
	if s.Clipboard {
		categories = append(categories, CategoryClipboard)
	}
	if s.Snippets {
		categories = append(categories, CategorySnippets)
	}
	if s.Notes {
		categories = append(categories, CategoryNotes)
	}
	if s.Learning {
		categories = append(categories, CategoryLearning)
	}
	return categories
}

// ClipboardRecord is one portable clip; the image is a bundle-relative name.
type ClipboardRecord struct {
	Kind      string `json:"kind"`
	Text      string `json:"text,omitempty"`
	Image     string `json:"image,omitempty"`
	CreatedAt int64  `json:"createdAt"`
	PinnedAt  int64  `json:"pinnedAt,omitempty"`
}

// Input is everything the composer hands to Export.
type Input struct {
	Version   string
	Selection Selection
	Settings  map[string]json.RawMessage
	Clipboard []ClipboardRecord
	Images    map[string][]byte
	Snippets  map[string][]byte
	Notes     map[string][]byte
	Learning  map[string]json.RawMessage
}

// Manifest is the table of contents.
type Manifest struct {
	Format     int            `json:"format"`
	AppVersion string         `json:"appVersion"`
	CreatedAt  int64          `json:"createdAt"`
	Counts     map[string]int `json:"counts"`
}

// Bundle is a parsed archive. Images are keyed by their clipboard record name.
type Bundle struct {
	Manifest  Manifest
	Settings  map[string]json.RawMessage
	Clipboard []ClipboardRecord
	Images    map[string][]byte
	Snippets  map[string][]byte
	Notes     map[string][]byte
	Learning  map[string]json.RawMessage
	Skipped   []string
}

const maxEntryBytes = 64 << 20
const maxTotalBytes = 512 << 20

// Export writes the selected categories as one ZIP archive.
func Export(in Input) ([]byte, error) {
	var buffer bytes.Buffer
	writer := zip.NewWriter(&buffer)
	counts := make(map[string]int)
	manifest := Manifest{Format: Format, AppVersion: in.Version, CreatedAt: time.Now().Unix(), Counts: counts}

	write := func(name string, data []byte) error {
		header := &zip.FileHeader{Name: name, Method: zip.Deflate}
		header.Modified = time.Now()
		part, err := writer.CreateHeader(header)
		if err != nil {
			return err
		}
		_, err = part.Write(data)
		return err
	}

	if in.Selection.Settings {
		settings := FilterSettings(in.Settings)
		data, err := json.MarshalIndent(settings, "", "  ")
		if err != nil {
			return nil, err
		}
		if err := write("settings.json", data); err != nil {
			return nil, err
		}
		counts[CategorySettings] = len(settings)
	}
	if in.Selection.Clipboard {
		var lines bytes.Buffer
		encoder := json.NewEncoder(&lines)
		for _, item := range in.Clipboard {
			if item.Kind == "image" {
				png, ok := in.Images[item.Image]
				if !ok || len(png) == 0 {
					return nil, fmt.Errorf("clipboard image %q is missing", item.Image)
				}
				if err := write("clipboard/images/"+item.Image, png); err != nil {
					return nil, err
				}
			}
			if err := encoder.Encode(item); err != nil {
				return nil, err
			}
		}
		if err := write("clipboard/items.jsonl", lines.Bytes()); err != nil {
			return nil, err
		}
		counts[CategoryClipboard] = len(in.Clipboard)
	}
	if in.Selection.Snippets {
		names := sortedKeys(in.Snippets)
		for _, name := range names {
			if err := write("snippets/"+name, in.Snippets[name]); err != nil {
				return nil, err
			}
		}
		counts[CategorySnippets] = len(names)
	}
	if in.Selection.Notes {
		names := sortedKeys(in.Notes)
		for _, name := range names {
			if err := write("notes/"+name, in.Notes[name]); err != nil {
				return nil, err
			}
		}
		counts[CategoryNotes] = len(names)
	}
	if in.Selection.Learning {
		names := sortedKeys(in.Learning)
		for _, name := range names {
			if err := write("learning/"+name+".json", in.Learning[name]); err != nil {
				return nil, err
			}
		}
		counts[CategoryLearning] = len(names)
	}
	data, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		return nil, err
	}
	if err := write("manifest.json", data); err != nil {
		return nil, err
	}
	if err := writer.Close(); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

// Read parses an archive, rejecting a different format and every unsafe path.
func Read(data []byte) (*Bundle, error) {
	reader, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil {
		return nil, fmt.Errorf("not a Tinycast backup: %w", err)
	}
	bundle := &Bundle{Images: make(map[string][]byte), Snippets: make(map[string][]byte),
		Notes: make(map[string][]byte), Learning: make(map[string]json.RawMessage)}
	var total int64
	for _, file := range reader.File {
		name := strings.ReplaceAll(file.Name, "\\", "/")
		if name == "" || path.IsAbs(name) || strings.HasPrefix(name, "../") || strings.Contains(name, "/../") {
			return nil, fmt.Errorf("unsafe archive entry %q", file.Name)
		}
		if file.FileInfo().IsDir() {
			continue
		}
		if file.UncompressedSize64 > maxEntryBytes {
			return nil, fmt.Errorf("archive entry %q is too large", file.Name)
		}
		total += int64(file.UncompressedSize64)
		if total > maxTotalBytes {
			return nil, errors.New("archive is too large")
		}
		content, err := readEntry(file)
		if err != nil {
			return nil, err
		}
		switch {
		case name == "manifest.json":
			if err := json.Unmarshal(content, &bundle.Manifest); err != nil {
				return nil, fmt.Errorf("read manifest: %w", err)
			}
		case name == "settings.json":
			if err := json.Unmarshal(content, &bundle.Settings); err != nil {
				return nil, fmt.Errorf("read settings: %w", err)
			}
		case name == "clipboard/items.jsonl":
			lines := strings.Split(string(content), "\n")
			for _, line := range lines {
				line = strings.TrimSpace(line)
				if line == "" {
					continue
				}
				var item ClipboardRecord
				if err := json.Unmarshal([]byte(line), &item); err != nil {
					return nil, fmt.Errorf("read clipboard item: %w", err)
				}
				bundle.Clipboard = append(bundle.Clipboard, item)
			}
		case strings.HasPrefix(name, "clipboard/images/"):
			bundle.Images[path.Base(name)] = content
		case strings.HasPrefix(name, "snippets/"):
			bundle.Snippets[path.Base(name)] = content
		case strings.HasPrefix(name, "notes/"):
			bundle.Notes[path.Base(name)] = content
		case strings.HasPrefix(name, "learning/"):
			base := path.Base(name)
			if strings.HasSuffix(base, ".json") {
				bundle.Learning[strings.TrimSuffix(base, ".json")] = content
			}
		}
	}
	if bundle.Manifest.Format != Format {
		return nil, fmt.Errorf("unsupported backup format %d", bundle.Manifest.Format)
	}
	for category := range bundle.Manifest.Counts {
		switch category {
		case CategorySettings, CategoryClipboard, CategorySnippets, CategoryNotes, CategoryLearning:
		default:
			bundle.Skipped = append(bundle.Skipped, category)
		}
	}
	bundle.Settings = FilterSettings(bundle.Settings)
	return bundle, nil
}

func readEntry(file *zip.File) ([]byte, error) {
	reader, err := file.Open()
	if err != nil {
		return nil, err
	}
	defer reader.Close()
	content, err := io.ReadAll(io.LimitReader(reader, maxEntryBytes+1))
	if err != nil {
		return nil, err
	}
	if int64(len(content)) > maxEntryBytes {
		return nil, fmt.Errorf("archive entry %q is too large", file.Name)
	}
	return content, nil
}

func sortedKeys[T any](values map[string]T) []string {
	names := make([]string, 0, len(values))
	for name := range values {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// excluded are capability grants, AI destinations/keys and machine-local
// geometry: a backup must never enable something the receiving user did not.
var excluded = map[string]bool{
	"snippetsEnabled":            true, // doubles as keystroke-listening consent
	"extensionsEnabled":          true, // doubles as third-party-code consent
	"extensionPackageManager":    true,
	"extensionRegistries":        true,
	"extensionCustomSearchPaths": true,
	"palettePosition":            true,
	"autoSwitchInputSource":      true,
	"calendarEnabled":            true,
	"autoJoinMeetings":           true,
	"cameraPreview":              true,
	"aiEnabled":                  true,
	"aiConnections":              true,
	"aiDefaultModel":             true,
	"aiWebSearch":                true,
	"aiSystemPrompt":             true,
	"aiSystemPromptEnabled":      true,
	"aiRetentionDays":            true,
	"aiOpensTo":                  true,
	"aiNewChatAfterMinutes":      true,
	"quickActionsEnabled":        true,
	"quickActionModel":           true,
	"quickActionPreviews":        true,
	"quickActionLanguage":        true,
	"notesWindow":                true,
	"notesActiveID":              true,
	"onboardingDone":             true,
}

// importable names the keys an imported file may write. It includes the
// original application's key spellings so an older file still applies.
var importable = map[string]bool{
	"appearance": true, "compactMode": true, "showFavoritesInCompactMode": true,
	"openOnCursorScreen": true, "paletteDraggable": true, "popToRootTimeout": true,
	"launchAtLogin": true, "showInMenuBar": true, "supportReminders": true,
	"extensionsShowInLauncher": true,
	"language":                 true, "uiLanguage": true, "hotkey": true, "snippetHotkeys": true,
	"dictionaryEnabled": true, "notesEnabled": true,
	"applicationsShowInLauncher": true, "systemSettingsShowInLauncher": true, "systemActionsShowInLauncher": true,
	"commandsEnabled": true, "customCommandsEnabled": true,
	"commandsShowInLauncher": true, "customCommandsShowInLauncher": true,
	"quicklinksEnabled": true, "quicklinksShowInLauncher": true, "quicklinkOpensNewWindow": true,
	"quicklinkSelectionFallback": true, "quicklinkConfirmsBeforeDelete": true,
	"snippetsShowInLauncher": true, "fileSearchEnabled": true, "fileSearchScopes": true,
	"fileSearchIgnorePatterns": true, "customAppDirs": true,
	"windowManagementEnabled": true, "windowManagementShowInLauncher": true,
	"windowManagementGap": true, "windowManagementCycleOnRepeat": true,
	"calendarShowInLauncher": true, "joinWindowMinutes": true, "autoJoinConfirms": true,
	"clipboardRetentionDays": true, "clipboardDisabledApps": true, "emojiSkinTone": true,
	"doubleTapModifier": true,
}

// FilterSettings removes excluded keys.
func FilterSettings(settings map[string]json.RawMessage) map[string]json.RawMessage {
	filtered := make(map[string]json.RawMessage, len(settings))
	for key, value := range settings {
		if excluded[key] {
			continue
		}
		filtered[key] = value
	}
	return filtered
}

// ApplyableSettings normalizes an imported settings object to the keys this
// build can apply, mapping the original application's older key spellings.
func ApplyableSettings(settings map[string]json.RawMessage) map[string]json.RawMessage {
	applied := make(map[string]json.RawMessage, len(settings))
	for key, value := range settings {
		if excluded[key] || !importable[key] {
			continue
		}
		switch key {
		case "customCommandsEnabled":
			applied["commandsEnabled"] = value
		case "customCommandsShowInLauncher":
			applied["commandsShowInLauncher"] = value
		case "uiLanguage":
			var raw string
			if json.Unmarshal(value, &raw) != nil {
				continue
			}
			if strings.HasPrefix(raw, "zh") {
				applied["language"] = json.RawMessage(`"zh"`)
			} else if raw == "en" {
				applied["language"] = json.RawMessage(`"en"`)
			}
		default:
			applied[key] = value
		}
	}
	return applied
}

// ExcludedReason explains why a key never travels, for the record and tests
// that do not exist; callers may surface it in the UI.
func ExcludedReason(key string) string {
	switch key {
	case "snippetsEnabled":
		return "keystroke-listening consent"
	case "extensionsEnabled":
		return "third-party-code consent"
	case "calendarEnabled", "autoJoinMeetings", "cameraPreview":
		return "calendar/camera access consent"
	case "quickActionsEnabled":
		return "keystroke delivery into other apps"
	case "aiEnabled", "aiConnections", "aiDefaultModel", "aiWebSearch", "aiSystemPrompt",
		"aiSystemPromptEnabled", "aiRetentionDays", "aiOpensTo", "aiNewChatAfterMinutes",
		"quickActionModel", "quickActionLanguage", "quickActionPreviews":
		return "AI destination or standing instruction"
	default:
		return ""
	}
}
