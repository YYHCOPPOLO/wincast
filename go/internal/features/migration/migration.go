// Package migration imports the legacy Windows application's data into the Go
// profile. It previews first, snapshots the target, stages and validates every
// converted file, and publishes only after the snapshot exists.
package migration

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

// Marker is the migration record written into the target profile.
const Marker = "migration.json"

// Target names the two writable roots of the current profile.
type Target struct {
	SettingsDir string
	DataDir     string
}

// Preview is the read-only inventory shown before a migration.
type Preview struct {
	Source         string
	SettingsFields int
	AIHistory      bool
	AIKeys         int
	Commands       int
	Quicklinks     int
	Snippets       int
	Notes          int
	ClipboardItems int
	Favorites      int
	Hidden         int
	Ranking        int
	Warnings       []string
}

// Result reports what the migration published.
type Result struct {
	Snapshot   string
	Settings   int
	AIHistory  bool
	AIKeys     int
	Commands   int
	Quicklinks int
	Snippets   int
	Notes      int
	Favorites  int
	Aliases    int
	Hidden     int
	Ranking    int
	Warnings   []string
}

// Options configures Apply. Resolve maps a legacy entry key to the current
// launcher's business ID; nil keeps only records that already match.
type Options struct {
	Source          string
	ClipboardSource string
	Target          Target
	Resolve         func(string) (string, bool)
	SkipAIHistory   bool
}

type legacy struct {
	root     string
	settings map[string]json.RawMessage
	info     Preview
}

// PreviewOf inventories the legacy profile without writing anything.
func PreviewOf(source, targetSettings, clipboardSource string) (Preview, error) {
	data, err := loadLegacy(source)
	if err != nil {
		return Preview{}, err
	}
	preview := data.info
	preview.Source = source
	if targetSettings != "" {
		if _, err := os.Stat(filepath.Join(targetSettings, "settings.json")); err == nil {
			preview.Warnings = append(preview.Warnings, "target settings.json exists; migrated fields will be merged")
		}
	}
	if clipboardSource != "" {
		if items, err := countClipboard(clipboardSource); err == nil {
			preview.ClipboardItems = items
		} else if !os.IsNotExist(err) {
			preview.Warnings = append(preview.Warnings, "legacy clipboard could not be read: "+err.Error())
		}
	}
	return preview, nil
}

func loadLegacy(source string) (*legacy, error) {
	info, err := os.Stat(source)
	if err != nil {
		return nil, fmt.Errorf("legacy data directory: %w", err)
	}
	if !info.IsDir() {
		return nil, errors.New("legacy source is not a directory")
	}
	data := &legacy{root: source, settings: make(map[string]json.RawMessage)}
	if raw, err := os.ReadFile(filepath.Join(source, "settings.json")); err == nil {
		if err := json.Unmarshal(raw, &data.settings); err != nil {
			return nil, fmt.Errorf("legacy settings.json: %w", err)
		}
		data.info.SettingsFields = len(legacySettings(data.settings))
	}
	data.info.AIHistory = fileExists(filepath.Join(source, "ai-chats.sqlite3"))
	if entries, err := os.ReadDir(filepath.Join(source, "ai-keys")); err == nil {
		for _, entry := range entries {
			if !entry.IsDir() && strings.HasSuffix(strings.ToLower(entry.Name()), ".dpapi") {
				data.info.AIKeys++
			}
		}
	}
	data.info.Commands = countJSONList(filepath.Join(source, "custom-commands.json"))
	data.info.Quicklinks = countJSONList(filepath.Join(source, "quicklinks.json"))
	data.info.Snippets = countMarkdown(filepath.Join(source, "Snippets"))
	data.info.Notes = countMarkdown(filepath.Join(source, "Notes"))
	data.info.Favorites, data.info.Hidden, data.info.Ranking = countLauncher(source)
	return data, nil
}

func countJSONList(path string) int {
	raw, err := os.ReadFile(path)
	if err != nil {
		return 0
	}
	var list []json.RawMessage
	if json.Unmarshal(raw, &list) == nil {
		return len(list)
	}
	var object map[string]json.RawMessage
	if json.Unmarshal(raw, &object) == nil {
		for _, key := range []string{"commands", "links", "items"} {
			if value, ok := object[key]; ok {
				if json.Unmarshal(value, &list) == nil {
					return len(list)
				}
			}
		}
	}
	return 0
}

func countMarkdown(dir string) int {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return 0
	}
	count := 0
	for _, entry := range entries {
		if !entry.IsDir() && strings.HasSuffix(strings.ToLower(entry.Name()), ".md") {
			count++
		}
	}
	return count
}

func countLauncher(source string) (favorites, hidden, ranking int) {
	var favoriteIDs []string
	if raw, err := os.ReadFile(filepath.Join(source, "favorites.json")); err == nil {
		_ = json.Unmarshal(raw, &favoriteIDs)
		favorites = len(favoriteIDs)
	}
	var visibility struct {
		HiddenItems []string `json:"hiddenItems"`
	}
	if raw, err := os.ReadFile(filepath.Join(source, "visibility.json")); err == nil {
		_ = json.Unmarshal(raw, &visibility)
		hidden = len(visibility.HiddenItems)
	}
	var records []domain.RankingRecord
	if raw, err := os.ReadFile(filepath.Join(source, "launcher-ranking.json")); err == nil {
		_ = json.Unmarshal(raw, &records)
		ranking = len(records)
	}
	return favorites, hidden, ranking
}

func countClipboard(source string) (int, error) {
	db, err := storage.OpenSQLite(context.Background(), filepath.Join(source, "clipboard.sqlite3"), true)
	if err != nil {
		return 0, err
	}
	defer db.Close()
	var count int
	if err := db.QueryRow("SELECT count(*) FROM items").Scan(&count); err != nil {
		return 0, err
	}
	return count, nil
}

// Apply snapshots, converts, validates and publishes the legacy data.
func Apply(opts Options) (Result, error) {
	data, err := loadLegacy(opts.Source)
	if err != nil {
		return Result{}, err
	}
	if samePath(opts.Source, opts.Target.SettingsDir) || samePath(opts.Source, opts.Target.DataDir) {
		return Result{}, errors.New("legacy source is the current profile")
	}
	stamp := time.Now().Format("20060102-150405")
	migrationDir := filepath.Join(opts.Target.SettingsDir, "migration")
	snapshot := filepath.Join(migrationDir, "snapshots", stamp)
	if err := snapshotTarget(opts.Target, snapshot); err != nil {
		return Result{}, fmt.Errorf("snapshot target: %w", err)
	}
	staging := filepath.Join(migrationDir, "staging-"+stamp)
	if err := os.RemoveAll(staging); err != nil {
		return Result{}, err
	}
	if err := os.MkdirAll(staging, 0o700); err != nil {
		return Result{}, err
	}
	result := Result{Snapshot: snapshot}
	if err := stageSettings(data, opts, staging, &result); err != nil {
		return Result{}, err
	}
	if err := stageAI(data, opts, staging, &result); err != nil {
		return Result{}, err
	}
	if err := stageLibraries(data, opts, staging, &result); err != nil {
		return Result{}, err
	}
	if err := stageMarkdown(data.root, opts, staging, "Snippets", "snippets", &result); err != nil {
		return Result{}, err
	}
	if err := stageMarkdown(data.root, opts, staging, "Notes", "notes", &result); err != nil {
		return Result{}, err
	}
	if err := stageSmallState(data.root, opts, staging, &result); err != nil {
		return Result{}, err
	}
	if err := stageLauncher(data, opts, staging, &result); err != nil {
		return Result{}, err
	}
	if err := writeMarker(staging, data, result); err != nil {
		return Result{}, err
	}
	if err := validateStaging(staging); err != nil {
		return Result{}, err
	}
	if err := publish(staging, opts.Target, &result); err != nil {
		return Result{}, err
	}
	return result, nil
}

// legacySettings renames the original application's key spellings and keeps
// every value, because a migration is a trusted one-time transfer rather than
// a portable backup: consent and AI configuration stay as the user set them.
func legacySettings(settings map[string]json.RawMessage) map[string]json.RawMessage {
	normalized := make(map[string]json.RawMessage, len(settings))
	for key, value := range settings {
		switch key {
		case "customCommandsEnabled":
			normalized["commandsEnabled"] = value
		case "customCommandsShowInLauncher":
			normalized["commandsShowInLauncher"] = value
		case "uiLanguage":
			var raw string
			if json.Unmarshal(value, &raw) != nil {
				continue
			}
			if strings.HasPrefix(raw, "zh") {
				normalized["language"] = json.RawMessage(`"zh"`)
			} else if raw == "en" {
				normalized["language"] = json.RawMessage(`"en"`)
			}
		default:
			normalized[key] = value
		}
	}
	return normalized
}

func stageSettings(data *legacy, opts Options, staging string, result *Result) error {
	applied := legacySettings(data.settings)
	target := make(map[string]json.RawMessage)
	if raw, err := os.ReadFile(filepath.Join(opts.Target.SettingsDir, "settings.json")); err == nil {
		if err := json.Unmarshal(raw, &target); err != nil {
			return fmt.Errorf("current settings.json: %w", err)
		}
	}
	for key, value := range applied {
		target[key] = value
		result.Settings++
	}
	if accelerator := legacyPaletteShortcut(filepath.Join(data.root, "hotkeys.json")); accelerator != "" {
		if _, exists := target["hotkey"]; !exists {
			encoded, err := json.Marshal(accelerator)
			if err != nil {
				return err
			}
			target["hotkey"] = encoded
		}
	}
	if len(target) == 0 {
		return nil
	}
	encoded, err := json.MarshalIndent(target, "", "  ")
	if err != nil {
		return err
	}
	return storage.WriteFileAtomic(filepath.Join(staging, "settings.json"), append(encoded, '\n'), 0o600)
}

func stageAI(data *legacy, opts Options, staging string, result *Result) error {
	if data.info.AIHistory {
		target := filepath.Join(opts.Target.SettingsDir, "ai-chats.sqlite3")
		if opts.SkipAIHistory {
			result.Warnings = append(result.Warnings, "AI is enabled; close AI before migrating its chat history")
		} else if !fileExists(target) {
			for _, name := range []string{"ai-chats.sqlite3", "ai-chats.sqlite3-wal", "ai-chats.sqlite3-shm"} {
				source := filepath.Join(data.root, name)
				if fileExists(source) {
					if err := copyFile(source, filepath.Join(staging, name)); err != nil {
						return err
					}
				}
			}
			result.AIHistory = true
		} else {
			result.Warnings = append(result.Warnings, "AI chat history already exists; kept the current file")
		}
	}
	entries, err := os.ReadDir(filepath.Join(data.root, "ai-keys"))
	if err != nil && !os.IsNotExist(err) {
		return err
	}
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(strings.ToLower(entry.Name()), ".dpapi") {
			continue
		}
		target := filepath.Join(opts.Target.SettingsDir, "ai-keys", entry.Name())
		if fileExists(target) {
			continue
		}
		if err := copyFile(filepath.Join(data.root, "ai-keys", entry.Name()), filepath.Join(staging, "ai-keys", entry.Name())); err != nil {
			return err
		}
		result.AIKeys++
	}
	return nil
}

func stageLibraries(data *legacy, opts Options, staging string, result *Result) error {
	for _, library := range []struct {
		name   string
		target string
		count  *int
	}{
		{"custom-commands.json", filepath.Join(opts.Target.SettingsDir, "custom-commands.json"), &result.Commands},
		{"quicklinks.json", filepath.Join(opts.Target.SettingsDir, "quicklinks.json"), &result.Quicklinks},
	} {
		source := filepath.Join(data.root, library.name)
		if !fileExists(source) {
			continue
		}
		if fileExists(library.target) {
			result.Warnings = append(result.Warnings, library.name+" already exists; kept the current file")
			continue
		}
		raw, err := os.ReadFile(source)
		if err != nil {
			return err
		}
		if !json.Valid(raw) {
			result.Warnings = append(result.Warnings, library.name+" is not valid JSON; skipped")
			continue
		}
		if err := storage.WriteFileAtomic(filepath.Join(staging, library.name), raw, 0o600); err != nil {
			return err
		}
		*library.count = 1
	}
	return nil
}

func stageMarkdown(root string, opts Options, staging, sourceName, targetName string, result *Result) error {
	entries, err := os.ReadDir(filepath.Join(root, sourceName))
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(strings.ToLower(entry.Name()), ".md") {
			continue
		}
		name := entry.Name()
		for suffix := 1; suffix <= 1000; suffix++ {
			candidate := name
			if suffix > 1 {
				stem := strings.TrimSuffix(name, filepath.Ext(name))
				candidate = fmt.Sprintf("%s-%d.md", stem, suffix)
			}
			if fileExists(filepath.Join(opts.Target.DataDir, targetName, candidate)) {
				continue
			}
			raw, err := os.ReadFile(filepath.Join(root, sourceName, name))
			if err != nil {
				return err
			}
			if err := storage.WriteFileAtomic(filepath.Join(staging, targetName, candidate), raw, 0o600); err != nil {
				return err
			}
			if targetName == "snippets" {
				result.Snippets++
			} else {
				result.Notes++
			}
			break
		}
	}
	return nil
}

// stageSmallState carries the one-time markers the current app reads from
// files: onboarding completion, the support reminder anchor and the calculator
// history. Existing target files are never replaced.
func stageSmallState(root string, opts Options, staging string, result *Result) error {
	for _, name := range []string{"onboarding-done", "support-reminder.json", "calculator-history.json"} {
		source := filepath.Join(root, name)
		target := filepath.Join(opts.Target.SettingsDir, name)
		if !fileExists(source) || fileExists(target) {
			continue
		}
		if err := copyFile(source, filepath.Join(staging, name)); err != nil {
			return err
		}
	}
	return nil
}

func stageLauncher(data *legacy, opts Options, staging string, result *Result) error {
	favorites := mapLegacyIDs(readStringList(filepath.Join(data.root, "favorites.json")), opts.Resolve)
	var visibility struct {
		HiddenItems []string `json:"hiddenItems"`
	}
	if raw, err := os.ReadFile(filepath.Join(data.root, "visibility.json")); err == nil {
		_ = json.Unmarshal(raw, &visibility)
	}
	hiddenIDs := mapLegacyIDs(visibility.HiddenItems, opts.Resolve)
	aliases := mapLegacyAliases(filepath.Join(data.root, "aliases.json"), opts.Resolve)
	var ranking []domain.RankingRecord
	if raw, err := os.ReadFile(filepath.Join(data.root, "launcher-ranking.json")); err == nil {
		var records []domain.RankingRecord
		if json.Unmarshal(raw, &records) == nil {
			for _, record := range records {
				if opts.Resolve == nil {
					continue
				}
				if mapped, ok := opts.Resolve(record.ItemKey); ok {
					record.ItemKey = mapped
					ranking = append(ranking, record)
				}
			}
		}
	}
	if len(favorites) > 0 {
		if err := writeStagedJSON(staging, "launcher/favorites.json", mergeStrings(readStringList(filepath.Join(opts.Target.SettingsDir, "launcher", "favorites.json")), favorites)); err != nil {
			return err
		}
		result.Favorites = len(favorites)
	}
	if len(aliases) > 0 {
		if err := writeStagedJSON(staging, "launcher/aliases.json", mergeAliases(readAliasMap(filepath.Join(opts.Target.SettingsDir, "launcher", "aliases.json")), aliases)); err != nil {
			return err
		}
		result.Aliases = len(aliases)
	}
	if len(hiddenIDs) > 0 {
		hidden := readHidden(filepath.Join(opts.Target.SettingsDir, "launcher", "hidden.json"))
		for _, id := range hiddenIDs {
			hidden[id] = true
		}
		if err := writeStagedJSON(staging, "launcher/hidden.json", hidden); err != nil {
			return err
		}
		result.Hidden = len(hiddenIDs)
	}
	if len(ranking) > 0 {
		merged := mergeRanking(readRanking(filepath.Join(opts.Target.SettingsDir, "launcher", "ranking.json")), ranking)
		if err := writeStagedJSON(staging, "launcher/ranking.json", merged); err != nil {
			return err
		}
		result.Ranking = len(ranking)
	}
	return nil
}

func writeStagedJSON(staging, name string, value any) error {
	encoded, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	return storage.WriteFileAtomic(filepath.Join(staging, name), append(encoded, '\n'), 0o600)
}

func writeMarker(staging string, data *legacy, result Result) error {
	marker := map[string]any{
		"format": 1, "source": data.root, "migratedAt": time.Now().Unix(),
		"settings": result.Settings, "aiHistory": result.AIHistory, "aiKeys": result.AIKeys,
		"snippets": result.Snippets, "notes": result.Notes,
		"favorites": result.Favorites, "aliases": result.Aliases, "hidden": result.Hidden,
		"ranking": result.Ranking,
	}
	return writeStagedJSON(staging, Marker, marker)
}

func validateStaging(staging string) error {
	if raw, err := os.ReadFile(filepath.Join(staging, "settings.json")); err == nil && !json.Valid(raw) {
		return errors.New("staged settings.json is not valid JSON")
	}
	if fileExists(filepath.Join(staging, "ai-chats.sqlite3")) {
		db, err := storage.OpenSQLite(context.Background(), filepath.Join(staging, "ai-chats.sqlite3"), true)
		if err != nil {
			return fmt.Errorf("staged AI history: %w", err)
		}
		defer db.Close()
		var check string
		if err := db.QueryRow("PRAGMA quick_check(1)").Scan(&check); err != nil {
			return fmt.Errorf("staged AI history: %w", err)
		}
		if check != "ok" {
			return fmt.Errorf("staged AI history: %s", check)
		}
	}
	return nil
}

func publish(staging string, target Target, result *Result) error {
	err := filepath.WalkDir(staging, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			return nil
		}
		relative, err := filepath.Rel(staging, path)
		if err != nil {
			return err
		}
		destination := filepath.Join(target.SettingsDir, relative)
		switch {
		case strings.HasPrefix(relative, "snippets"+string(filepath.Separator)):
			destination = filepath.Join(target.DataDir, "snippets", filepath.Base(relative))
		case strings.HasPrefix(relative, "notes"+string(filepath.Separator)):
			destination = filepath.Join(target.DataDir, "notes", filepath.Base(relative))
		case strings.HasPrefix(relative, "launcher"+string(filepath.Separator)):
			destination = filepath.Join(target.SettingsDir, relative)
		}
		if fileExists(destination) && relative != Marker &&
			relative != "settings.json" &&
			!strings.HasPrefix(relative, "launcher"+string(filepath.Separator)) {
			return nil
		}
		raw, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		return storage.WriteFileAtomic(destination, raw, 0o600)
	})
	if err != nil {
		return err
	}
	marker, err := os.ReadFile(filepath.Join(staging, Marker))
	if err != nil {
		return err
	}
	if err := storage.WriteFileAtomic(filepath.Join(target.SettingsDir, Marker), marker, 0o600); err != nil {
		return err
	}
	return os.RemoveAll(staging)
}

// Snapshot describes one target snapshot directory.
type Snapshot struct {
	Path string
	At   time.Time
}

// Snapshots lists target snapshots, newest first. A snapshot holds settings/
// and data/ trees plus a manifest.
func Snapshots(settingsDir string) ([]Snapshot, error) {
	dir := filepath.Join(settingsDir, "migration", "snapshots")
	entries, err := os.ReadDir(dir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	var snapshots []Snapshot
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		info, err := entry.Info()
		if err != nil {
			continue
		}
		snapshots = append(snapshots, Snapshot{Path: filepath.Join(dir, entry.Name()), At: info.ModTime()})
	}
	sort.SliceStable(snapshots, func(i, j int) bool { return snapshots[i].At.After(snapshots[j].At) })
	return snapshots, nil
}

// Restore copies a snapshot back over the target. It never deletes files that
// were created after the snapshot; those need the data export path instead.
func Restore(settingsDir, dataDir, snapshot string) error {
	info, err := os.Stat(snapshot)
	if err != nil {
		return err
	}
	if !info.IsDir() {
		return errors.New("snapshot is not a directory")
	}
	for _, pair := range []struct{ source, target string }{
		{filepath.Join(snapshot, "settings"), settingsDir},
		{filepath.Join(snapshot, "data"), dataDir},
	} {
		if err := copyTree(pair.source, pair.target); err != nil {
			return err
		}
	}
	return nil
}

func snapshotTarget(target Target, snapshot string) error {
	for _, pair := range []struct{ source, target string }{
		{target.SettingsDir, filepath.Join(snapshot, "settings")},
		{target.DataDir, filepath.Join(snapshot, "data")},
	} {
		if err := copyTreeFiltered(pair.source, pair.target, filepath.Join(target.SettingsDir, "migration")); err != nil {
			return err
		}
	}
	return writeSnapshotManifest(snapshot)
}

func writeSnapshotManifest(snapshot string) error {
	manifest := map[string]any{"format": 1, "createdAt": time.Now().Unix()}
	encoded, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		return err
	}
	return storage.WriteFileAtomic(filepath.Join(snapshot, "manifest.json"), append(encoded, '\n'), 0o600)
}

func copyTree(source, target string) error {
	return copyTreeFiltered(source, target, "")
}

func copyTreeFiltered(source, target, skip string) error {
	info, err := os.Stat(source)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	if !info.IsDir() {
		return nil
	}
	return filepath.WalkDir(source, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if skip != "" && samePath(path, skip) {
			return filepath.SkipDir
		}
		relative, err := filepath.Rel(source, path)
		if err != nil {
			return err
		}
		destination := filepath.Join(target, relative)
		if entry.IsDir() {
			return os.MkdirAll(destination, 0o700)
		}
		return copyFile(path, destination)
	})
}

func copyFile(source, destination string) error {
	raw, err := os.ReadFile(source)
	if err != nil {
		return err
	}
	return storage.WriteFileAtomic(destination, raw, 0o600)
}

func fileExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular()
}

func samePath(left, right string) bool {
	leftAbs, errLeft := filepath.Abs(left)
	rightAbs, errRight := filepath.Abs(right)
	if errLeft != nil || errRight != nil {
		return strings.EqualFold(filepath.Clean(left), filepath.Clean(right))
	}
	return strings.EqualFold(filepath.Clean(leftAbs), filepath.Clean(rightAbs))
}

func readStringList(path string) []string {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	var list []string
	_ = json.Unmarshal(raw, &list)
	return list
}

func readAliasMap(path string) map[string]string {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	var aliases map[string]string
	_ = json.Unmarshal(raw, &aliases)
	return aliases
}

func readHidden(path string) map[string]bool {
	raw, err := os.ReadFile(path)
	if err != nil {
		return make(map[string]bool)
	}
	var hidden map[string]bool
	if json.Unmarshal(raw, &hidden) != nil || hidden == nil {
		return make(map[string]bool)
	}
	return hidden
}

func readRanking(path string) []domain.RankingRecord {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	var records []domain.RankingRecord
	_ = json.Unmarshal(raw, &records)
	return records
}

func mapLegacyIDs(ids []string, resolve func(string) (string, bool)) []string {
	if resolve == nil {
		return nil
	}
	seen := make(map[string]bool, len(ids))
	var mapped []string
	for _, id := range ids {
		if current, ok := resolve(id); ok && !seen[current] {
			seen[current] = true
			mapped = append(mapped, current)
		}
	}
	return mapped
}

func mapLegacyAliases(path string, resolve func(string) (string, bool)) map[string]string {
	if resolve == nil {
		return nil
	}
	source := readAliasMap(path)
	mapped := make(map[string]string, len(source))
	for id, alias := range source {
		if current, ok := resolve(id); ok {
			mapped[current] = alias
		}
	}
	return mapped
}

func mergeStrings(existing, incoming []string) []string {
	seen := make(map[string]bool, len(existing)+len(incoming))
	result := make([]string, 0, len(existing)+len(incoming))
	for _, id := range append(append([]string(nil), existing...), incoming...) {
		if id == "" || seen[id] {
			continue
		}
		seen[id] = true
		result = append(result, id)
	}
	return result
}

func mergeAliases(existing, incoming map[string]string) map[string]string {
	result := make(map[string]string, len(existing)+len(incoming))
	for id, alias := range existing {
		result[id] = alias
	}
	for id, alias := range incoming {
		if _, ok := result[id]; !ok {
			result[id] = alias
		}
	}
	return result
}

func mergeRanking(existing, incoming []domain.RankingRecord) []domain.RankingRecord {
	type key struct{ query, item string }
	merged := make(map[key]domain.RankingRecord, len(existing)+len(incoming))
	order := make([]key, 0, len(existing)+len(incoming))
	add := func(record domain.RankingRecord) {
		index := key{record.Query, record.ItemKey}
		current, ok := merged[index]
		if !ok {
			order = append(order, index)
		}
		if !ok || record.Count > current.Count || record.LastUsed > current.LastUsed {
			merged[index] = record
		}
	}
	for _, record := range existing {
		add(record)
	}
	for _, record := range incoming {
		add(record)
	}
	result := make([]domain.RankingRecord, 0, len(order))
	for _, index := range order {
		result = append(result, merged[index])
	}
	return result
}

// legacyPaletteShortcut converts the legacy hotkey file's toggle binding to
// the accelerator text the current settings store uses.
func legacyPaletteShortcut(path string) string {
	raw, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	var bindings map[string]struct {
		Combo struct {
			VK        int `json:"vk"`
			Modifiers struct {
				Ctrl  bool `json:"ctrl"`
				Alt   bool `json:"alt"`
				Shift bool `json:"shift"`
				Win   bool `json:"win"`
			} `json:"modifiers"`
		} `json:"Combo"`
	}
	if json.Unmarshal(raw, &bindings) != nil {
		return ""
	}
	binding, ok := bindings["hotkey.togglePalette"]
	if !ok {
		return ""
	}
	key := virtualKeyName(binding.Combo.VK)
	if key == "" {
		return ""
	}
	var parts []string
	if binding.Combo.Modifiers.Ctrl {
		parts = append(parts, "Ctrl")
	}
	if binding.Combo.Modifiers.Alt {
		parts = append(parts, "Alt")
	}
	if binding.Combo.Modifiers.Shift {
		parts = append(parts, "Shift")
	}
	if binding.Combo.Modifiers.Win {
		parts = append(parts, "Win")
	}
	parts = append(parts, key)
	return strings.Join(parts, "+")
}

func virtualKeyName(vk int) string {
	switch vk {
	case 32:
		return "Space"
	case 13:
		return "Enter"
	case 27:
		return "Esc"
	case 9:
		return "Tab"
	case 192:
		return "`"
	}
	if vk >= 'A' && vk <= 'Z' {
		return string(rune(vk))
	}
	if vk >= '0' && vk <= '9' {
		return string(rune(vk))
	}
	if vk >= 0x70 && vk <= 0x87 {
		return fmt.Sprintf("F%d", vk-0x6F)
	}
	return ""
}
