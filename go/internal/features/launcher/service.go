package launcher

import (
	"context"
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
)

type Service struct {
	apps   []domain.Application
	fields map[string]domain.SearchFields
	prefs  *Preferences
}

func New(prefs *Preferences) *Service {
	return &Service{fields: make(map[string]domain.SearchFields), prefs: prefs}
}

func (s *Service) Preferences() *Preferences { return s.prefs }

// Applications returns the deduplicated catalog currently shown by the
// launcher. The slice must not be mutated by callers.
func (s *Service) Applications() []domain.Application { return s.apps }

// SetApps replaces the catalog. Entries with the same business id or the same
// case-insensitive path are deduplicated; the first occurrence wins because
// the AppsFolder catalog is authoritative over custom directories.
func (s *Service) SetApps(apps []domain.Application) {
	seenID := make(map[string]bool, len(apps))
	seenPath := make(map[string]bool, len(apps))
	clean := make([]domain.Application, 0, len(apps))
	fields := make(map[string]domain.SearchFields, len(apps))
	for _, app := range apps {
		key := app.ID
		if key == "" {
			key = strings.ToLower(app.Path)
		}
		pathKey := strings.ToLower(filepath.Clean(app.Path))
		if seenID[key] || (pathKey != "" && seenPath[pathKey]) {
			continue
		}
		seenID[key], seenPath[pathKey] = true, true
		clean = append(clean, app)
		fields[key] = domain.SearchFields{
			Alias:      s.prefs.Alias(app.ID),
			Display:    app.Name,
			BundleID:   bundleID(app.ID),
			Executable: executableName(app.Path),
		}
	}
	s.apps = clean
	s.fields = fields
}

func bundleID(id string) string {
	if id == "" || strings.ContainsAny(id, `\/:`) {
		return ""
	}
	return id
}

func executableName(path string) string {
	base := filepath.Base(path)
	return strings.TrimSuffix(base, filepath.Ext(base))
}

func (s *Service) Application(id string) (domain.Application, bool) {
	for _, app := range s.apps {
		if app.ID == id {
			return app, true
		}
	}
	return domain.Application{}, false
}

// Search ranks the current catalog. The catalog is small enough to rank on
// the UI thread; the caller measures and reports the filter cost.
func (s *Service) Search(query string, showHidden bool, now time.Time) []domain.Application {
	query = strings.TrimSpace(query)
	if query == "" {
		return s.emptyQuery(showHidden)
	}
	type scored struct {
		app   domain.Application
		score int
	}
	results := make([]scored, 0, len(s.apps))
	for _, app := range s.apps {
		if !showHidden && s.prefs.IsHidden(app.ID) {
			continue
		}
		key := app.ID
		if key == "" {
			key = strings.ToLower(app.Path)
		}
		fields := s.fields[key]
		score, ok := domain.SearchScore(query, fields)
		if !ok {
			continue
		}
		results = append(results, scored{app: app, score: score + s.prefs.Boost(query, key, now)})
	}
	sort.SliceStable(results, func(i, j int) bool {
		if results[i].score != results[j].score {
			return results[i].score > results[j].score
		}
		left, right := strings.ToLower(results[i].app.Name), strings.ToLower(results[j].app.Name)
		if left != right {
			return left < right
		}
		return results[i].app.Path < results[j].app.Path
	})
	apps := make([]domain.Application, len(results))
	for i, result := range results {
		apps[i] = result.app
	}
	return apps
}

func (s *Service) emptyQuery(showHidden bool) []domain.Application {
	apps := make([]domain.Application, 0, len(s.apps))
	added := make(map[string]bool, len(s.apps))
	for _, id := range s.prefs.FavoriteOrder() {
		app, ok := s.Application(id)
		if !ok || (!showHidden && s.prefs.IsHidden(id)) {
			continue
		}
		apps = append(apps, app)
		added[app.ID] = true
	}
	rest := make([]domain.Application, 0, len(s.apps))
	for _, app := range s.apps {
		if added[app.ID] || (!showHidden && s.prefs.IsHidden(app.ID)) {
			continue
		}
		rest = append(rest, app)
	}
	sort.SliceStable(rest, func(i, j int) bool {
		left, right := strings.ToLower(rest[i].Name), strings.ToLower(rest[j].Name)
		if left != right {
			return left < right
		}
		return rest[i].Path < rest[j].Path
	})
	return append(apps, rest...)
}

func (s *Service) Record(query, id string, now time.Time) {
	query = strings.TrimSpace(query)
	if query == "" || id == "" {
		return
	}
	s.prefs.Record(query, id, now)
}

// ScanCustomApps adds executables and shortcuts under user-chosen directories.
// It never follows a directory above depth two or a reparse point directory.
func ScanCustomApps(ctx context.Context, directories []string) ([]domain.Application, error) {
	var apps []domain.Application
	var firstErr error
	for _, directory := range directories {
		if err := ctx.Err(); err != nil {
			return apps, err
		}
		directory = filepath.Clean(directory)
		info, err := filepath.Abs(directory)
		if err != nil {
			if firstErr == nil {
				firstErr = err
			}
			continue
		}
		rootDepth := strings.Count(info, string(filepath.Separator))
		err = filepath.WalkDir(info, func(path string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return nil
			}
			if err := ctx.Err(); err != nil {
				return err
			}
			if entry.IsDir() {
				if path != info && strings.Count(path, string(filepath.Separator))-rootDepth >= 2 {
					return filepath.SkipDir
				}
				return nil
			}
			extension := strings.ToLower(filepath.Ext(path))
			if extension != ".exe" && extension != ".lnk" {
				return nil
			}
			name := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
			if name == "" {
				return nil
			}
			apps = append(apps, domain.Application{ID: "custom:" + path, Name: name, Path: path})
			return nil
		})
		if err != nil && firstErr == nil {
			firstErr = err
		}
	}
	return apps, firstErr
}

// ValidateDirectories checks that configured entries are directories; missing
// paths are reported instead of silently ignored.
func ValidateDirectories(directories []string) error {
	for _, directory := range directories {
		info, err := os.Stat(filepath.Clean(directory))
		if err != nil || !info.IsDir() {
			return errors.New("custom application directory is missing: " + directory)
		}
	}
	return nil
}
