package storage

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"slices"
)

// Document is a JSON object that keeps keys it does not model. Settings from a
// newer build therefore survive an older build's save instead of being
// silently dropped. The zero Document is empty, not nil.
type Document struct {
	values map[string]json.RawMessage
}

func NewDocument() *Document {
	return &Document{values: make(map[string]json.RawMessage)}
}

// LoadDocument reads an existing object. A missing file is an empty document;
// invalid JSON is an error, so callers never overwrite a damaged file with
// defaults.
func LoadDocument(path string) (*Document, error) {
	data, err := os.ReadFile(path)
	if errors.Is(err, fs.ErrNotExist) {
		return NewDocument(), nil
	}
	if err != nil {
		return nil, err
	}
	document := NewDocument()
	if err := json.Unmarshal(bytes.TrimSpace(data), &document.values); err != nil {
		return nil, fmt.Errorf("read settings document: %w", err)
	}
	if document.values == nil {
		document.values = make(map[string]json.RawMessage)
	}
	return document, nil
}

func (d *Document) Keys() []string {
	keys := make([]string, 0, len(d.values))
	for key := range d.values {
		keys = append(keys, key)
	}
	slices.Sort(keys)
	return keys
}

func (d *Document) Has(key string) bool {
	_, ok := d.values[key]
	return ok
}

func (d *Document) Raw(key string) (json.RawMessage, bool) {
	value, ok := d.values[key]
	return value, ok
}

func (d *Document) String(key, fallback string) string {
	var value string
	if raw, ok := d.values[key]; ok {
		if json.Unmarshal(raw, &value) == nil {
			return value
		}
	}
	return fallback
}

func (d *Document) Bool(key string, fallback bool) bool {
	var value bool
	if raw, ok := d.values[key]; ok {
		if json.Unmarshal(raw, &value) == nil {
			return value
		}
	}
	return fallback
}

func (d *Document) Int(key string, fallback int) int {
	var value int
	if raw, ok := d.values[key]; ok {
		if json.Unmarshal(raw, &value) == nil {
			return value
		}
	}
	return fallback
}

func (d *Document) Set(key string, value any) error {
	raw, err := json.Marshal(value)
	if err != nil {
		return err
	}
	d.values[key] = raw
	return nil
}

func (d *Document) Delete(key string) {
	delete(d.values, key)
}

// Encode returns the complete document bytes without writing them. Callers
// that hand the bytes to a worker keep the Document itself on the UI thread.
func (d *Document) Encode() ([]byte, error) {
	data, err := json.MarshalIndent(d.values, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(data, '\n'), nil
}

// Save writes a complete replacement atomically: the old file remains in
// place until the new bytes have reached disk and the rename succeeds.
func (d *Document) Save(path string) error {
	data, err := d.Encode()
	if err != nil {
		return err
	}
	return WriteFileAtomic(path, data, 0o600)
}

// WriteFileAtomic replaces path only after the temporary file is synced and
// closed. A failure anywhere before the rename leaves the original untouched.
func WriteFileAtomic(path string, data []byte, mode fs.FileMode) error {
	directory := filepath.Dir(path)
	if err := os.MkdirAll(directory, 0o700); err != nil {
		return err
	}
	file, err := os.CreateTemp(directory, ".tinycast-*.tmp")
	if err != nil {
		return err
	}
	temporary := file.Name()
	cleanup := func() {
		file.Close()
		os.Remove(temporary)
	}
	if _, err := file.Write(data); err != nil {
		cleanup()
		return err
	}
	if err := file.Sync(); err != nil {
		cleanup()
		return err
	}
	if err := file.Close(); err != nil {
		os.Remove(temporary)
		return err
	}
	if err := os.Chmod(temporary, mode); err != nil {
		os.Remove(temporary)
		return err
	}
	if err := os.Rename(temporary, path); err != nil {
		os.Remove(temporary)
		return err
	}
	return nil
}
