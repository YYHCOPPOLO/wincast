// Package snippets owns the Markdown snippet library: one file per snippet,
// atomic writes, revision-checked saves and user-editable frontmatter.
package snippets

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/snippet"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

var (
	// ErrConflict reports that the file changed after it was loaded.
	ErrConflict = errors.New("the snippet changed on disk")
	// ErrFileNotFound reports a deleted or renamed snippet file.
	ErrFileNotFound = errors.New("the snippet file no longer exists")
	// ErrInvalidLocation reports a path outside this channel's library.
	ErrInvalidLocation = errors.New("the snippet file is outside this library")
)

// Issue is one file that could not be parsed. Valid files stay available.
type Issue struct {
	Path    string
	Message string
}

// Snapshot is one consistent listing of the library.
type Snapshot struct {
	Records []snippet.Snippet
	Issues  []Issue
}

// Repository reads and writes one snippet directory.
type Repository struct {
	dir string
}

func NewRepository(dir string) *Repository { return &Repository{dir: dir} }

func (r *Repository) Directory() string { return r.dir }

// Load lists every regular Markdown child. Malformed files are reported and
// never hidden; the valid records remain usable.
func (r *Repository) Load() (Snapshot, error) {
	if err := os.MkdirAll(r.dir, 0o700); err != nil {
		return Snapshot{}, fmt.Errorf("open snippet library: %w", err)
	}
	entries, err := os.ReadDir(r.dir)
	if err != nil {
		return Snapshot{}, fmt.Errorf("read snippet library: %w", err)
	}
	snapshot := Snapshot{}
	for _, entry := range entries {
		if entry.IsDir() || !strings.EqualFold(filepath.Ext(entry.Name()), ".md") {
			continue
		}
		path := filepath.Join(r.dir, entry.Name())
		info, err := entry.Info()
		if err != nil || !info.Mode().IsRegular() {
			continue
		}
		data, err := os.ReadFile(path)
		if err != nil {
			snapshot.Issues = append(snapshot.Issues, Issue{Path: path, Message: err.Error()})
			continue
		}
		record, err := snippet.ParseMarkdown(path, string(data))
		if err != nil {
			snapshot.Issues = append(snapshot.Issues, Issue{Path: path, Message: err.Error()})
			continue
		}
		snapshot.Records = append(snapshot.Records, record)
	}
	sort.SliceStable(snapshot.Records, func(i, j int) bool {
		left, right := strings.ToLower(snapshot.Records[i].Name), strings.ToLower(snapshot.Records[j].Name)
		if left != right {
			return left < right
		}
		return snapshot.Records[i].ID < snapshot.Records[j].ID
	})
	sort.SliceStable(snapshot.Issues, func(i, j int) bool { return snapshot.Issues[i].Path < snapshot.Issues[j].Path })
	return snapshot, nil
}

// Create writes a new file. The filename is a slug with a numeric suffix, so
// a duplicate name never overwrites an existing snippet.
func (r *Repository) Create(name, keyword string, enabled, showConfirmation bool, body string) (snippet.Snippet, error) {
	if err := os.MkdirAll(r.dir, 0o700); err != nil {
		return snippet.Snippet{}, fmt.Errorf("open snippet library: %w", err)
	}
	draft := snippet.Snippet{Name: strings.TrimSpace(name), Keyword: strings.TrimSpace(keyword),
		Enabled: enabled, ShowConfirmation: showConfirmation, Body: body}
	if draft.Name == "" {
		return snippet.Snippet{}, errors.New("a snippet needs a name")
	}
	content := snippet.Serialize(draft)
	base := snippet.Slug(draft.Name)
	for suffix := 1; suffix <= 10000; suffix++ {
		name := base + ".md"
		if suffix > 1 {
			name = fmt.Sprintf("%s-%d.md", base, suffix)
		}
		path := filepath.Join(r.dir, name)
		handle, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err != nil {
			if errors.Is(err, fs.ErrExist) {
				continue
			}
			return snippet.Snippet{}, fmt.Errorf("create snippet: %w", err)
		}
		_, writeErr := handle.WriteString(content)
		closeErr := handle.Close()
		if writeErr != nil || closeErr != nil {
			os.Remove(path)
			return snippet.Snippet{}, fmt.Errorf("create snippet: %w", errors.Join(writeErr, closeErr))
		}
		draft.ID = path
		draft.Revision = snippet.RevisionOf(content)
		return draft, nil
	}
	return snippet.Snippet{}, errors.New("could not claim a snippet file name")
}

// Save replaces one existing file only when the loaded revision still matches.
func (r *Repository) Save(record snippet.Snippet) (snippet.Snippet, error) {
	path, err := r.validated(record.ID)
	if err != nil {
		return snippet.Snippet{}, err
	}
	current, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return snippet.Snippet{}, ErrFileNotFound
		}
		return snippet.Snippet{}, fmt.Errorf("read snippet: %w", err)
	}
	if snippet.RevisionOf(string(current)) != record.Revision {
		return snippet.Snippet{}, ErrConflict
	}
	content := snippet.Serialize(record)
	if err := storage.WriteFileAtomic(path, []byte(content), 0o600); err != nil {
		return snippet.Snippet{}, fmt.Errorf("save snippet: %w", err)
	}
	record.ID = path
	record.Revision = snippet.RevisionOf(content)
	return record, nil
}

// Delete removes one file only when the loaded revision still matches.
func (r *Repository) Delete(record snippet.Snippet) error {
	path, err := r.validated(record.ID)
	if err != nil {
		return err
	}
	current, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return ErrFileNotFound
		}
		return fmt.Errorf("read snippet: %w", err)
	}
	if snippet.RevisionOf(string(current)) != record.Revision {
		return ErrConflict
	}
	if err := os.Remove(path); err != nil {
		return fmt.Errorf("delete snippet: %w", err)
	}
	return nil
}

func (r *Repository) validated(path string) (string, error) {
	absolute, err := filepath.Abs(path)
	if err != nil {
		return "", ErrInvalidLocation
	}
	root, err := filepath.Abs(r.dir)
	if err != nil {
		return "", ErrInvalidLocation
	}
	if !strings.EqualFold(filepath.Dir(absolute), root) ||
		!strings.EqualFold(filepath.Ext(absolute), ".md") ||
		strings.HasPrefix(filepath.Base(absolute), ".") {
		return "", ErrInvalidLocation
	}
	return absolute, nil
}
