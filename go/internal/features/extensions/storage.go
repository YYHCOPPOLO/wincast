package extensions

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/extension"
)

// Storage keeps one JSON file per extension: LocalStorage, cache namespaces and
// user-set preferences. Files, not SQLite: the Cache API is synchronous, so the
// whole namespace is read at launch, and an extension's store is small.
type Storage struct {
	dir   string
	mu    sync.Mutex
	store map[string]*storageFile
	dirty map[string]bool
	timer *time.Timer
}

type storageFile struct {
	LocalStorage map[string]any               `json:"localStorage"`
	Caches       map[string]map[string]string `json:"caches"`
	Preferences  map[string]any               `json:"preferences"`
	Internal     map[string]any               `json:"internal,omitempty"`
}

// NewStorage opens the extension-data directory.
func NewStorage(directory string) *Storage {
	_ = os.MkdirAll(directory, 0o700)
	return &Storage{
		dir:   directory,
		store: make(map[string]*storageFile),
		dirty: make(map[string]bool),
	}
}

// SafeName flattens an npm-style name into one path segment. This is the only
// copy of the mapping: a second one that drifts orphans every file the first
// one wrote.
func SafeName(name string) string {
	name = strings.ReplaceAll(name, "/", "-")
	return strings.ReplaceAll(name, "@", "")
}

func (s *Storage) fileURL(extensionName string) string {
	return filepath.Join(s.dir, SafeName(extensionName)+".json")
}

func (s *Storage) file(extensionName string) *storageFile {
	if existing := s.store[extensionName]; existing != nil {
		return existing
	}
	loaded := &storageFile{}
	if data, err := os.ReadFile(s.fileURL(extensionName)); err == nil {
		_ = json.Unmarshal(data, loaded)
	}
	if loaded.LocalStorage == nil {
		loaded.LocalStorage = make(map[string]any)
	}
	if loaded.Caches == nil {
		loaded.Caches = make(map[string]map[string]string)
	}
	if loaded.Preferences == nil {
		loaded.Preferences = make(map[string]any)
	}
	if loaded.Internal == nil {
		loaded.Internal = make(map[string]any)
	}
	s.store[extensionName] = loaded
	return loaded
}

func (s *Storage) mutate(extensionName string, body func(*storageFile)) {
	s.mu.Lock()
	defer s.mu.Unlock()
	file := s.file(extensionName)
	body(file)
	s.dirty[extensionName] = true
	s.scheduleFlush()
}

func (s *Storage) scheduleFlush() {
	if s.timer != nil {
		return
	}
	s.timer = time.AfterFunc(250*time.Millisecond, s.Flush)
}

// Flush writes every dirty extension store atomically.
func (s *Storage) Flush() {
	s.mu.Lock()
	pending := make(map[string]*storageFile, len(s.dirty))
	for name := range s.dirty {
		pending[name] = s.store[name]
	}
	s.dirty = make(map[string]bool)
	if s.timer != nil {
		s.timer.Stop()
		s.timer = nil
	}
	s.mu.Unlock()
	for name, file := range pending {
		data, err := json.Marshal(file)
		if err != nil {
			continue
		}
		_ = writeFileAtomic(s.fileURL(name), data)
	}
}

// LocalGet returns a LocalStorage value, or nil.
func (s *Storage) LocalGet(extensionName, key string) any {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.file(extensionName).LocalStorage[key]
}

// LocalAll returns every LocalStorage value.
func (s *Storage) LocalAll(extensionName string) map[string]any {
	s.mu.Lock()
	defer s.mu.Unlock()
	file := s.file(extensionName)
	all := make(map[string]any, len(file.LocalStorage))
	for key, value := range file.LocalStorage {
		all[key] = value
	}
	return all
}

// LocalSet stores a string, number or boolean.
func (s *Storage) LocalSet(extensionName, key string, value any) {
	switch value.(type) {
	case string, float64, int, bool:
	default:
		return
	}
	s.mutate(extensionName, func(file *storageFile) { file.LocalStorage[key] = value })
}

// LocalRemove deletes one key.
func (s *Storage) LocalRemove(extensionName, key string) {
	s.mutate(extensionName, func(file *storageFile) { delete(file.LocalStorage, key) })
}

// LocalClear empties LocalStorage.
func (s *Storage) LocalClear(extensionName string) {
	s.mutate(extensionName, func(file *storageFile) {
		file.LocalStorage = make(map[string]any)
	})
}

// Caches returns every cache namespace. The Cache API is synchronous, so the
// runtime receives this snapshot at launch.
func (s *Storage) Caches(extensionName string) map[string]map[string]string {
	s.mu.Lock()
	defer s.mu.Unlock()
	file := s.file(extensionName)
	caches := make(map[string]map[string]string, len(file.Caches))
	for namespace, bucket := range file.Caches {
		copied := make(map[string]string, len(bucket))
		for key, value := range bucket {
			copied[key] = value
		}
		caches[namespace] = copied
	}
	return caches
}

// CacheSet writes one cache entry. A nil key clears the namespace; a nil value
// removes one entry.
func (s *Storage) CacheSet(extensionName, namespace string, key, value *string) {
	s.mutate(extensionName, func(file *storageFile) {
		if key == nil {
			file.Caches[namespace] = map[string]string{}
			return
		}
		bucket := file.Caches[namespace]
		if bucket == nil {
			bucket = make(map[string]string)
			file.Caches[namespace] = bucket
		}
		if value == nil {
			delete(bucket, *key)
		} else {
			bucket[*key] = *value
		}
	})
}

// CacheClear empties one namespace.
func (s *Storage) CacheClear(extensionName, namespace string) {
	s.mutate(extensionName, func(file *storageFile) {
		file.Caches[namespace] = map[string]string{}
	})
}

// Preference returns a user-set preference; manifest defaults are applied by
// ResolvePreferences.
func (s *Storage) Preference(extensionName, key string) (any, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	value, ok := s.file(extensionName).Preferences[key]
	return value, ok
}

// SetPreference writes or clears a preference.
func (s *Storage) SetPreference(extensionName, key string, value any) {
	s.mutate(extensionName, func(file *storageFile) {
		if value == nil {
			delete(file.Preferences, key)
			return
		}
		file.Preferences[key] = value
	})
}

// AllPreferences returns the user-set values.
func (s *Storage) AllPreferences(extensionName string) map[string]any {
	s.mu.Lock()
	defer s.mu.Unlock()
	file := s.file(extensionName)
	values := make(map[string]any, len(file.Preferences))
	for key, value := range file.Preferences {
		values[key] = value
	}
	return values
}

// ResolvePreferences overlays user values on manifest defaults.
func (s *Storage) ResolvePreferences(extensionName string, schemas []extension.Preference) map[string]extension.PreferenceValue {
	resolved := make(map[string]extension.PreferenceValue, len(schemas))
	for _, schema := range schemas {
		resolved[schema.Name] = schema.EffectiveDefault()
	}
	for _, schema := range schemas {
		if raw, ok := s.Preference(extensionName, schema.Name); ok {
			if value, ok := extension.FromJSON(raw); ok {
				resolved[schema.Name] = value
			}
		}
	}
	return resolved
}

// MissingRequiredPreferences lists required preferences that still have no
// value; a command must not run with one unset.
func (s *Storage) MissingRequiredPreferences(extensionName string, schemas []extension.Preference) []extension.Preference {
	missing := make([]extension.Preference, 0)
	for _, schema := range schemas {
		if !schema.Required {
			continue
		}
		value := schema.EffectiveDefault()
		if raw, ok := s.Preference(extensionName, schema.Name); ok {
			if parsed, ok := extension.FromJSON(raw); ok {
				value = parsed
			}
		}
		if value.Kind == extension.ValueString && value.String == "" {
			missing = append(missing, schema)
		}
	}
	return missing
}

// InternalGet returns a runtime-internal value (dropdown/form drafts).
func (s *Storage) InternalGet(extensionName, key string) (any, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	value, ok := s.file(extensionName).Internal[key]
	return value, ok
}

// InternalSet stores a runtime-internal value.
func (s *Storage) InternalSet(extensionName, key string, value any) {
	s.mutate(extensionName, func(file *storageFile) {
		if file.Internal == nil {
			file.Internal = make(map[string]any)
		}
		file.Internal[key] = value
	})
}

// InternalRemove deletes a runtime-internal value.
func (s *Storage) InternalRemove(extensionName, key string) {
	s.mutate(extensionName, func(file *storageFile) {
		delete(file.Internal, key)
	})
}

// RemoveAll forgets one extension's file and memory state.
func (s *Storage) RemoveAll(extensionName string) {
	s.mu.Lock()
	delete(s.store, extensionName)
	delete(s.dirty, extensionName)
	s.mu.Unlock()
	_ = os.Remove(s.fileURL(extensionName))
}

func writeFileAtomic(path string, data []byte) error {
	directory := filepath.Dir(path)
	if err := os.MkdirAll(directory, 0o700); err != nil {
		return err
	}
	temporary, err := os.CreateTemp(directory, ".wincast-*.tmp")
	if err != nil {
		return err
	}
	name := temporary.Name()
	defer os.Remove(name)
	if _, err := temporary.Write(data); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Sync(); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	return os.Rename(name, path)
}

// sortedKeys is used by the settings editor and diagnostics.
func sortedKeys(values map[string]any) []string {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}
