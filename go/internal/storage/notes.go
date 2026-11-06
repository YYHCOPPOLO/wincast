package storage

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// ErrNoteConflict reports that a note changed on disk after it was loaded.
// Callers must keep their unsaved text and let the user decide; they must not
// overwrite the external revision silently.
var ErrNoteConflict = errors.New("note changed on disk")

type Note struct {
	ID       string
	Title    string
	Body     string
	Modified time.Time
}

type NoteSummary struct {
	ID       string
	Title    string
	Modified time.Time
}

// NoteRepository stores Markdown source verbatim. Implementations must keep
// unknown front matter and line endings intact.
type NoteRepository interface {
	List(ctx context.Context) ([]NoteSummary, error)
	Load(ctx context.Context, id string) (Note, error)
	Save(ctx context.Context, note Note) (Note, error)
	Delete(ctx context.Context, id string) error
}

// FileNotes keeps one Markdown file per note under root. The identifier is the
// file stem, never a path supplied by the caller.
type FileNotes struct {
	root string
}

func NewFileNotes(root string) *FileNotes {
	return &FileNotes{root: root}
}

func (f *FileNotes) path(id string) (string, error) {
	if !safeNoteID(id) {
		return "", fmt.Errorf("invalid note id %q", id)
	}
	return filepath.Join(f.root, id+".md"), nil
}

func safeNoteID(id string) bool {
	if id == "" || len(id) > 128 {
		return false
	}
	for _, r := range id {
		switch {
		case r >= 'a' && r <= 'z', r >= 'A' && r <= 'Z', r >= '0' && r <= '9', r == '-', r == '_':
		default:
			return false
		}
	}
	return true
}

func (f *FileNotes) List(ctx context.Context) ([]NoteSummary, error) {
	entries, err := os.ReadDir(f.root)
	if errors.Is(err, fs.ErrNotExist) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	notes := make([]NoteSummary, 0, len(entries))
	for _, entry := range entries {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		if entry.IsDir() || !strings.EqualFold(filepath.Ext(entry.Name()), ".md") {
			continue
		}
		id := strings.TrimSuffix(entry.Name(), filepath.Ext(entry.Name()))
		if !safeNoteID(id) {
			continue
		}
		info, err := entry.Info()
		if err != nil {
			continue
		}
		title := id
		if data, err := os.ReadFile(filepath.Join(f.root, entry.Name())); err == nil {
			if heading := noteTitle(data); heading != "" {
				title = heading
			}
		}
		notes = append(notes, NoteSummary{ID: id, Title: title, Modified: info.ModTime()})
	}
	sort.Slice(notes, func(i, j int) bool {
		if notes[i].Modified.Equal(notes[j].Modified) {
			return notes[i].ID < notes[j].ID
		}
		return notes[i].Modified.After(notes[j].Modified)
	})
	return notes, nil
}

func noteTitle(body []byte) string {
	for _, line := range strings.Split(string(body), "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "# ") {
			return strings.TrimSpace(strings.TrimPrefix(line, "# "))
		}
	}
	return ""
}

func (f *FileNotes) Load(ctx context.Context, id string) (Note, error) {
	if err := ctx.Err(); err != nil {
		return Note{}, err
	}
	path, err := f.path(id)
	if err != nil {
		return Note{}, err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return Note{}, err
	}
	info, err := os.Stat(path)
	if err != nil {
		return Note{}, err
	}
	return Note{ID: id, Title: noteTitle(data), Body: string(data), Modified: info.ModTime()}, nil
}

func (f *FileNotes) Save(ctx context.Context, note Note) (Note, error) {
	if err := ctx.Err(); err != nil {
		return Note{}, err
	}
	if err := os.MkdirAll(f.root, 0o700); err != nil {
		return Note{}, err
	}
	id := note.ID
	if id == "" {
		generated, err := NewID()
		if err != nil {
			return Note{}, err
		}
		id = generated
	}
	path, err := f.path(id)
	if err != nil {
		return Note{}, err
	}
	if info, err := os.Stat(path); err == nil {
		if note.Modified.IsZero() || !info.ModTime().Equal(note.Modified) {
			return Note{}, ErrNoteConflict
		}
	} else if !errors.Is(err, fs.ErrNotExist) {
		return Note{}, err
	}
	if err := WriteFileAtomic(path, []byte(note.Body), 0o600); err != nil {
		return Note{}, err
	}
	info, err := os.Stat(path)
	if err != nil {
		return Note{}, err
	}
	return Note{ID: id, Title: noteTitle([]byte(note.Body)), Body: note.Body, Modified: info.ModTime()}, nil
}

func (f *FileNotes) Delete(ctx context.Context, id string) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	path, err := f.path(id)
	if err != nil {
		return err
	}
	if err := os.Remove(path); err != nil && !errors.Is(err, fs.ErrNotExist) {
		return err
	}
	return nil
}
