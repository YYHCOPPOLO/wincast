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

// Note is one Markdown file. ID is the file name, the title is its stem and
// the body is the literal source.
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

// NoteRepository stores Markdown source verbatim. Implementations keep the
// file name as identity and never rewrite the body.
type NoteRepository interface {
	List(ctx context.Context) ([]NoteSummary, error)
	Load(ctx context.Context, id string) (Note, error)
	Save(ctx context.Context, note Note) (Note, error)
	ForceSave(ctx context.Context, note Note) (Note, error)
	Create(ctx context.Context) (Note, error)
	Delete(ctx context.Context, id string) error
	Modified(ctx context.Context, id string) (time.Time, error)
}

// FileNotes keeps one Markdown file per note under root.
type FileNotes struct {
	root string
}

func NewFileNotes(root string) *FileNotes {
	return &FileNotes{root: root}
}

func (f *FileNotes) Directory() string { return f.root }

func (f *FileNotes) path(id string) (string, error) {
	if !safeNoteName(id) {
		return "", fmt.Errorf("invalid note name %q", id)
	}
	return filepath.Join(f.root, id), nil
}

// safeNoteName accepts an immediate Markdown child only. Titles may contain
// spaces and non-ASCII letters; path separators and parent references never.
func safeNoteName(id string) bool {
	if id == "" || id == "." || id == ".." || len(id) > 240 {
		return false
	}
	if id != filepath.Base(id) || strings.ContainsAny(id, `/\:`) || strings.HasPrefix(id, ".") {
		return false
	}
	return strings.EqualFold(filepath.Ext(id), ".md")
}

func noteTitle(id string) string {
	return strings.TrimSuffix(filepath.Base(id), filepath.Ext(id))
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
		if entry.IsDir() || !safeNoteName(entry.Name()) {
			continue
		}
		info, err := entry.Info()
		if err != nil || !info.Mode().IsRegular() {
			continue
		}
		notes = append(notes, NoteSummary{ID: entry.Name(), Title: noteTitle(entry.Name()), Modified: info.ModTime()})
	}
	sort.SliceStable(notes, func(i, j int) bool {
		if notes[i].Modified.Equal(notes[j].Modified) {
			return strings.ToLower(notes[i].Title) < strings.ToLower(notes[j].Title)
		}
		return notes[i].Modified.After(notes[j].Modified)
	})
	return notes, nil
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
	return Note{ID: id, Title: noteTitle(id), Body: string(data), Modified: info.ModTime()}, nil
}

// Save writes only when the loaded revision is still on disk.
func (f *FileNotes) Save(ctx context.Context, note Note) (Note, error) {
	if err := ctx.Err(); err != nil {
		return Note{}, err
	}
	if err := os.MkdirAll(f.root, 0o700); err != nil {
		return Note{}, err
	}
	path, err := f.path(note.ID)
	if err != nil {
		return Note{}, err
	}
	info, statErr := os.Stat(path)
	switch {
	case statErr == nil:
		if note.Modified.IsZero() || !info.ModTime().Equal(note.Modified) {
			return Note{}, ErrNoteConflict
		}
	case errors.Is(statErr, fs.ErrNotExist):
		if !note.Modified.IsZero() {
			return Note{}, ErrNoteConflict
		}
	default:
		return Note{}, statErr
	}
	return f.write(path, note)
}

// ForceSave replaces the file after the user explicitly chose their draft
// over the external revision.
func (f *FileNotes) ForceSave(ctx context.Context, note Note) (Note, error) {
	if err := ctx.Err(); err != nil {
		return Note{}, err
	}
	if err := os.MkdirAll(f.root, 0o700); err != nil {
		return Note{}, err
	}
	path, err := f.path(note.ID)
	if err != nil {
		return Note{}, err
	}
	return f.write(path, note)
}

func (f *FileNotes) write(path string, note Note) (Note, error) {
	if err := WriteFileAtomic(path, []byte(note.Body), 0o600); err != nil {
		return Note{}, err
	}
	info, err := os.Stat(path)
	if err != nil {
		return Note{}, err
	}
	return Note{ID: note.ID, Title: noteTitle(note.ID), Body: note.Body, Modified: info.ModTime()}, nil
}

// Create claims the first free Untitled name. An empty library creates
// Untitled.md; later notes receive " 2", " 3" and so on.
func (f *FileNotes) Create(ctx context.Context) (Note, error) {
	if err := ctx.Err(); err != nil {
		return Note{}, err
	}
	if err := os.MkdirAll(f.root, 0o700); err != nil {
		return Note{}, err
	}
	for suffix := 1; suffix <= 10000; suffix++ {
		name := "Untitled.md"
		if suffix > 1 {
			name = fmt.Sprintf("Untitled %d.md", suffix)
		}
		path := filepath.Join(f.root, name)
		handle, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err != nil {
			if errors.Is(err, fs.ErrExist) {
				continue
			}
			return Note{}, err
		}
		if err := handle.Close(); err != nil {
			os.Remove(path)
			return Note{}, err
		}
		info, err := os.Stat(path)
		if err != nil {
			return Note{}, err
		}
		return Note{ID: name, Title: noteTitle(name), Modified: info.ModTime()}, nil
	}
	return Note{}, errors.New("could not claim a note name")
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

func (f *FileNotes) Modified(ctx context.Context, id string) (time.Time, error) {
	if err := ctx.Err(); err != nil {
		return time.Time{}, err
	}
	path, err := f.path(id)
	if err != nil {
		return time.Time{}, err
	}
	info, err := os.Stat(path)
	if err != nil {
		return time.Time{}, err
	}
	return info.ModTime(), nil
}
