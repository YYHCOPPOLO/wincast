// Package clipboard stores the clipboard history shared by the preview and
// daily identities. The schema matches the Rust application: a text UUID is
// the stable business id, rowid is recency, and image rows keep absolute PNG
// paths so a legacy store can be moved without losing its files.
package clipboard

import (
	"context"
	"crypto/rand"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"sync"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

const (
	memoryWindow = 1000
	ftsLimit     = 200
)

type Kind uint8

const (
	KindText Kind = iota
	KindImage
)

type Filter uint8

const (
	FilterAll Filter = iota
	FilterText
	FilterImages
	FilterLinks
	FilterEmails
)

type Item struct {
	ID        string
	Kind      Kind
	Text      string
	ImagePath string
	CreatedAt int64
	PinnedAt  int64
}

func (i Item) Pinned() bool { return i.PinnedAt > 0 }

type Store struct {
	mu      sync.Mutex
	db      *sql.DB
	dir     string
	images  string
	items   []Item
	maxAge  int64
	lastErr error
	closed  bool
}

// Open opens the store under dir, optionally copying a legacy store first when
// the destination has no database yet. The source is left in place as a backup.
func Open(ctx context.Context, dir, legacy string) (*Store, error) {
	store := &Store{dir: dir, images: filepath.Join(dir, "images"), maxAge: -1}
	store.mu.Lock()
	defer store.mu.Unlock()
	if err := store.openLocked(ctx, legacy); err != nil {
		return nil, err
	}
	return store, nil
}

func (s *Store) openLocked(ctx context.Context, legacy string) error {
	if hasParentComponent(s.dir) {
		return errors.New("invalid clipboard storage directory")
	}
	if err := os.MkdirAll(s.dir, 0o700); err != nil {
		return err
	}
	if err := checkedDirectory(s.dir); err != nil {
		return err
	}
	if legacy != "" && legacy != s.dir {
		if err := migrateStore(legacy, s.dir); err != nil {
			// A failed copy must never replace a readable old history; keep
			// using the legacy location instead.
			if !fileExists(filepath.Join(s.dir, "clipboard.sqlite3")) &&
				fileExists(filepath.Join(legacy, "clipboard.sqlite3")) {
				s.dir = legacy
				s.images = filepath.Join(legacy, "images")
			}
		}
	}
	path := filepath.Join(s.dir, "clipboard.sqlite3")
	existing := fileExists(path)
	if existing {
		if err := validateSidecars(path); err != nil {
			return err
		}
	}
	database, err := storage.OpenSQLite(ctx, path, false)
	if err != nil {
		return err
	}
	if existing {
		if err := validateSchema(ctx, database); err != nil {
			database.Close()
			return err
		}
	} else if err := createSchema(ctx, database); err != nil {
		database.Close()
		return err
	}
	if err := os.MkdirAll(s.images, 0o700); err != nil {
		database.Close()
		return err
	}
	if err := checkedDirectory(s.images); err != nil {
		database.Close()
		return err
	}
	items, err := loadItems(ctx, database)
	if err != nil {
		database.Close()
		return err
	}
	s.db = database
	s.items = items
	return nil
}

func hasParentComponent(path string) bool {
	for _, part := range strings.FieldsFunc(filepath.Clean(path), func(r rune) bool {
		return r == '\\' || r == '/'
	}) {
		if part == ".." {
			return true
		}
	}
	return false
}

func fileExists(path string) bool {
	info, err := os.Lstat(path)
	return err == nil && info.Mode().IsRegular()
}

func checkedDirectory(path string) error {
	info, err := os.Lstat(path)
	if err != nil {
		return err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("invalid clipboard directory: %s", path)
	}
	return nil
}

func validateSidecars(path string) error {
	info, err := os.Lstat(path)
	if err != nil || !info.Mode().IsRegular() {
		return errors.New("invalid clipboard database path")
	}
	for _, suffix := range []string{"-wal", "-shm", "-journal"} {
		sidecar := path + suffix
		sidecarInfo, err := os.Lstat(sidecar)
		if errors.Is(err, os.ErrNotExist) {
			continue
		}
		if err != nil {
			return err
		}
		if suffix == "-journal" || !sidecarInfo.Mode().IsRegular() ||
			sidecarInfo.Mode()&os.ModeSymlink != 0 {
			return errors.New("clipboard database has unsupported sidecars")
		}
	}
	return nil
}

func createSchema(ctx context.Context, db *sql.DB) error {
	_, err := db.ExecContext(ctx, `
PRAGMA journal_mode=WAL;
CREATE TABLE IF NOT EXISTS items(
  rowid INTEGER PRIMARY KEY AUTOINCREMENT,
  id TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  pinned_at INTEGER,
  kind TEXT NOT NULL CHECK (kind IN ('text','image')),
  text TEXT,
  image_path TEXT
);
CREATE INDEX IF NOT EXISTS items_created_at ON items(created_at);
CREATE INDEX IF NOT EXISTS items_pinned_at ON items(pinned_at) WHERE pinned_at IS NOT NULL;
CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
  text, content='items', content_rowid='rowid', tokenize='trigram'
);
CREATE TRIGGER IF NOT EXISTS items_ai AFTER INSERT ON items BEGIN
  INSERT INTO items_fts(rowid, text) VALUES(new.rowid, new.text);
END;
CREATE TRIGGER IF NOT EXISTS items_ad AFTER DELETE ON items BEGIN
  INSERT INTO items_fts(items_fts, rowid, text) VALUES('delete', old.rowid, old.text);
END;
`)
	return err
}

func validateSchema(ctx context.Context, db *sql.DB) error {
	var version int64
	if err := db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		return fmt.Errorf("read clipboard database version: %w", err)
	}
	var createSQL string
	if err := db.QueryRowContext(ctx,
		"SELECT sql FROM sqlite_master WHERE type='table' AND name='items'").Scan(&createSQL); err != nil {
		return err
	}
	if version != 0 || !strings.Contains(strings.ToLower(createSQL), "autoincrement") {
		return errors.New("unsupported clipboard database schema")
	}
	rows, err := db.QueryContext(ctx, "PRAGMA table_info(items)")
	if err != nil {
		return err
	}
	defer rows.Close()
	type column struct {
		name string
		kind string
		pk   int
	}
	var columns []column
	for rows.Next() {
		var cid, notNull, pk int
		var name, kind string
		var fallback any
		if err := rows.Scan(&cid, &name, &kind, &notNull, &fallback, &pk); err != nil {
			return err
		}
		columns = append(columns, column{name: name, kind: kind, pk: pk})
	}
	if err := rows.Err(); err != nil {
		return err
	}
	rows.Close()
	expected := []column{
		{name: "rowid", kind: "INTEGER", pk: 1},
		{name: "id", kind: "TEXT"},
		{name: "created_at", kind: "INTEGER"},
		{name: "pinned_at", kind: "INTEGER"},
		{name: "kind", kind: "TEXT"},
		{name: "text", kind: "TEXT"},
		{name: "image_path", kind: "TEXT"},
	}
	if len(columns) != len(expected) {
		return errors.New("unsupported clipboard database schema")
	}
	for i, expectedColumn := range expected {
		if columns[i].name != expectedColumn.name ||
			!strings.EqualFold(columns[i].kind, expectedColumn.kind) ||
			columns[i].pk != expectedColumn.pk {
			return errors.New("unsupported clipboard database schema")
		}
	}
	var objects int
	if err := db.QueryRowContext(ctx, `SELECT COUNT(*) FROM sqlite_master
		WHERE (type='table' AND name='items_fts')
		   OR (type='trigger' AND name IN ('items_ai','items_ad'))`).Scan(&objects); err != nil {
		return err
	}
	if objects != 3 {
		return errors.New("incomplete clipboard database schema")
	}
	var check string
	if err := db.QueryRowContext(ctx, "PRAGMA quick_check(1)").Scan(&check); err != nil {
		return err
	}
	if check != "ok" {
		return errors.New(check)
	}
	_, err = loadItems(ctx, db)
	return err
}

type queryer interface {
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func loadItems(ctx context.Context, q queryer) ([]Item, error) {
	var floor int64
	err := q.QueryRowContext(ctx, `SELECT rowid FROM items WHERE pinned_at IS NULL
		ORDER BY rowid DESC LIMIT 1 OFFSET ?`, memoryWindow-1).Scan(&floor)
	if err != nil && !errors.Is(err, sql.ErrNoRows) {
		return nil, err
	}
	rows, err := q.QueryContext(ctx, `SELECT id, created_at, pinned_at, kind, text, image_path FROM (
		SELECT rowid AS rid, * FROM items WHERE rowid >= ?
		UNION ALL
		SELECT rowid AS rid, * FROM items WHERE pinned_at IS NOT NULL AND rowid < ?
	) ORDER BY rid DESC`, floor, floor)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var items []Item
	for rows.Next() {
		item, err := scanItem(rows)
		if err != nil {
			return nil, err
		}
		items = append(items, item)
	}
	return items, rows.Err()
}

type rowScanner interface {
	Scan(dest ...any) error
}

func scanItem(row rowScanner) (Item, error) {
	var item Item
	var kind string
	var text, imagePath sql.NullString
	var pinned sql.NullInt64
	if err := row.Scan(&item.ID, &item.CreatedAt, &pinned, &kind, &text, &imagePath); err != nil {
		return Item{}, err
	}
	if kind == "image" {
		item.Kind = KindImage
	}
	item.Text = text.String
	item.ImagePath = imagePath.String
	if pinned.Valid {
		item.PinnedAt = pinned.Int64
	}
	return item, nil
}

func (s *Store) Available() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.db != nil && !s.closed
}

func (s *Store) LastError() string {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lastErr == nil {
		return ""
	}
	return s.lastErr.Error()
}

func (s *Store) ImagesDir() string { return s.images }

func (s *Store) SetMaxAgeSeconds(seconds int64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.maxAge = seconds
}

func (s *Store) InsertText(text string) (string, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.db == nil || s.closed {
		return "", false
	}
	if len(s.items) > 0 {
		first := s.items[0]
		if first.Kind == KindText && first.Text == text {
			s.lastErr = nil
			return first.ID, true
		}
	}
	now := time.Now().Unix()
	id := newID()
	if err := s.execInsertLocked("INSERT INTO items(id, created_at, pinned_at, kind, text, image_path) VALUES (?, ?, NULL, 'text', ?, NULL)",
		id, now, text); err != nil {
		return "", false
	}
	return id, true
}

func (s *Store) InsertImage(path string) (string, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.db == nil || s.closed {
		return "", false
	}
	now := time.Now().Unix()
	id := newID()
	if err := s.execInsertLocked("INSERT INTO items(id, created_at, pinned_at, kind, text, image_path) VALUES (?, ?, NULL, 'image', NULL, ?)",
		id, now, path); err != nil {
		return "", false
	}
	return id, true
}

// SaveImage writes an encoded PNG beside the database and commits its row.
// The file is created exclusively and removed if the row cannot be inserted.
func (s *Store) SaveImage(png []byte) (string, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.db == nil || s.closed {
		return "", false
	}
	if err := checkedDirectory(s.images); err != nil {
		s.lastErr = err
		return "", false
	}
	path := filepath.Join(s.images, newID()+".png")
	file, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		s.lastErr = err
		return "", false
	}
	if _, err := file.Write(png); err != nil {
		file.Close()
		os.Remove(path)
		s.lastErr = err
		return "", false
	}
	if err := file.Sync(); err != nil {
		file.Close()
		os.Remove(path)
		s.lastErr = err
		return "", false
	}
	if err := file.Close(); err != nil {
		os.Remove(path)
		s.lastErr = err
		return "", false
	}
	now := time.Now().Unix()
	id := newID()
	if err := s.execInsertLocked("INSERT INTO items(id, created_at, pinned_at, kind, text, image_path) VALUES (?, ?, NULL, 'image', NULL, ?)",
		id, now, path); err != nil {
		os.Remove(path)
		return "", false
	}
	return id, true
}

func (s *Store) execInsertLocked(statement string, args ...any) error {
	if err := s.withTransactionLocked(func(tx *sql.Tx) error {
		result, err := tx.Exec(statement, args...)
		if err != nil {
			return err
		}
		affected, err := result.RowsAffected()
		if err != nil {
			return err
		}
		if affected != 1 {
			return fmt.Errorf("clipboard insert affected %d rows", affected)
		}
		return nil
	}); err != nil {
		s.lastErr = err
		return err
	}
	s.lastErr = nil
	if s.maxAge >= 0 {
		s.pruneLocked(s.maxAge)
	}
	return nil
}

func (s *Store) withTransactionLocked(operation func(*sql.Tx) error) error {
	if s.db == nil {
		return errors.New("clipboard store is unavailable")
	}
	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	if err := operation(tx); err != nil {
		tx.Rollback()
		return err
	}
	items, err := loadItems(context.Background(), tx)
	if err != nil {
		tx.Rollback()
		return err
	}
	if err := tx.Commit(); err != nil {
		return err
	}
	s.items = items
	return nil
}

func (s *Store) Search(query string, filter Filter) []Item {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.db == nil || s.closed {
		return nil
	}
	query = strings.TrimSpace(query)
	var rows []Item
	switch {
	case query == "":
		rows = s.orderedLocked()
	case len([]rune(query)) < 3:
		rows = s.fallbackLocked(query)
	default:
		rows = s.ftsLocked(query)
	}
	return applyFilter(rows, filter)
}

func (s *Store) orderedLocked() []Item {
	pins := make([]Item, 0, len(s.items))
	rest := make([]Item, 0, len(s.items))
	for _, item := range s.items {
		if item.Pinned() {
			pins = append(pins, item)
		} else {
			rest = append(rest, item)
		}
	}
	slices.SortStableFunc(pins, func(a, b Item) int {
		switch {
		case a.PinnedAt < b.PinnedAt:
			return -1
		case a.PinnedAt > b.PinnedAt:
			return 1
		default:
			return 0
		}
	})
	return append(pins, rest...)
}

func (s *Store) fallbackLocked(query string) []Item {
	needle := strings.ToLower(query)
	matches := func(item Item) bool {
		return item.Kind == KindText && strings.Contains(strings.ToLower(item.Text), needle)
	}
	pins := make([]Item, 0)
	rest := make([]Item, 0)
	for _, item := range s.items {
		if !matches(item) {
			continue
		}
		if item.Pinned() {
			pins = append(pins, item)
		} else {
			rest = append(rest, item)
		}
	}
	slices.SortStableFunc(pins, func(a, b Item) int {
		switch {
		case a.PinnedAt < b.PinnedAt:
			return -1
		case a.PinnedAt > b.PinnedAt:
			return 1
		default:
			return 0
		}
	})
	return append(pins, rest...)
}

func (s *Store) ftsLocked(query string) []Item {
	pins := make([]Item, 0)
	for _, item := range s.items {
		if item.Pinned() && containsFold(item.Text, query) {
			pins = append(pins, item)
		}
	}
	slices.SortStableFunc(pins, func(a, b Item) int {
		switch {
		case a.PinnedAt < b.PinnedAt:
			return -1
		case a.PinnedAt > b.PinnedAt:
			return 1
		default:
			return 0
		}
	})
	match := `"` + strings.ReplaceAll(query, `"`, `""`) + `"`
	rows, err := s.db.Query(`SELECT items.id, items.created_at, items.pinned_at, items.kind, items.text, items.image_path
		FROM items JOIN items_fts ON items.rowid = items_fts.rowid
		WHERE items_fts MATCH ? AND items.pinned_at IS NULL
		ORDER BY items.rowid DESC LIMIT ?`, match, ftsLimit)
	if err != nil {
		return s.fallbackLocked(query)
	}
	defer rows.Close()
	seen := make(map[string]bool, len(pins))
	for _, item := range pins {
		seen[item.ID] = true
	}
	for rows.Next() {
		item, err := scanItem(rows)
		if err != nil {
			return s.fallbackLocked(query)
		}
		if !seen[item.ID] {
			pins = append(pins, item)
			seen[item.ID] = true
		}
	}
	return pins
}

func containsFold(text, query string) bool {
	return strings.Contains(strings.ToLower(text), strings.ToLower(query))
}

func applyFilter(rows []Item, filter Filter) []Item {
	result := rows[:0]
	for _, item := range rows {
		keep := false
		switch filter {
		case FilterAll:
			keep = true
		case FilterImages:
			keep = item.Kind == KindImage
		case FilterText:
			keep = item.Kind == KindText && domain.ClassifyClipboardText(item.Text) == domain.ClipTextPlain
		case FilterLinks:
			keep = item.Kind == KindText && domain.ClassifyClipboardText(item.Text) == domain.ClipTextLink
		case FilterEmails:
			keep = item.Kind == KindText && domain.ClassifyClipboardText(item.Text) == domain.ClipTextEmail
		}
		if keep {
			result = append(result, item)
		}
	}
	return result
}

func (s *Store) TogglePin(id string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	item, ok := s.findLocked(id)
	if !ok {
		return false
	}
	if item.Pinned() {
		createdAt := time.Now().Unix()
		if item.CreatedAt+1 > createdAt {
			createdAt = item.CreatedAt + 1
		}
		return s.reinsertLocked(item, createdAt, 0)
	}
	stamp := s.nextPinStampLocked()
	err := s.withTransactionLocked(func(tx *sql.Tx) error {
		result, err := tx.Exec("UPDATE items SET pinned_at = ? WHERE id = ?", stamp, id)
		if err != nil {
			return err
		}
		affected, err := result.RowsAffected()
		if err != nil {
			return err
		}
		if affected != 1 {
			return fmt.Errorf("clipboard pin affected %d rows", affected)
		}
		return nil
	})
	if err != nil {
		s.lastErr = err
		return false
	}
	s.lastErr = nil
	return true
}

func (s *Store) Promote(id string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	item, ok := s.findLocked(id)
	if !ok || item.Pinned() {
		return false
	}
	if len(s.items) > 0 && s.items[0].ID == id {
		return true
	}
	createdAt := time.Now().Unix()
	if item.CreatedAt+1 > createdAt {
		createdAt = item.CreatedAt + 1
	}
	return s.reinsertLocked(item, createdAt, 0)
}

func (s *Store) reinsertLocked(item Item, createdAt, pinnedAt int64) bool {
	kind := "text"
	if item.Kind == KindImage {
		kind = "image"
	}
	var text, imagePath any
	if item.Kind == KindText {
		text = item.Text
	} else {
		imagePath = item.ImagePath
	}
	err := s.withTransactionLocked(func(tx *sql.Tx) error {
		result, err := tx.Exec("DELETE FROM items WHERE id = ?", item.ID)
		if err != nil {
			return err
		}
		affected, err := result.RowsAffected()
		if err != nil || affected != 1 {
			return errors.New("clipboard row could not be replaced")
		}
		var pin any
		if pinnedAt > 0 {
			pin = pinnedAt
		}
		_, err = tx.Exec("INSERT INTO items(id, created_at, pinned_at, kind, text, image_path) VALUES (?, ?, ?, ?, ?, ?)",
			item.ID, createdAt, pin, kind, text, imagePath)
		return err
	})
	if err != nil {
		s.lastErr = err
		return false
	}
	s.lastErr = nil
	return true
}

func (s *Store) findLocked(id string) (Item, bool) {
	for _, item := range s.items {
		if item.ID == id {
			return item, true
		}
	}
	if s.db == nil {
		return Item{}, false
	}
	row := s.db.QueryRow("SELECT id, created_at, pinned_at, kind, text, image_path FROM items WHERE id = ?", id)
	item, err := scanItem(row)
	if err != nil {
		return Item{}, false
	}
	return item, true
}

func (s *Store) nextPinStampLocked() int64 {
	now := time.Now().Unix()
	last := int64(0)
	for _, item := range s.items {
		if item.PinnedAt > last {
			last = item.PinnedAt
		}
	}
	if last+1 > now {
		return last + 1
	}
	return now
}

func (s *Store) RowIndex(id, query string, filter Filter) int {
	rows := s.Search(query, filter)
	for index, item := range rows {
		if item.ID == id {
			return index
		}
	}
	return -1
}

func (s *Store) PinnedItem(index int, query string, filter Filter) (Item, bool) {
	rows := s.Search(query, filter)
	pinned := 0
	for _, item := range rows {
		if item.Pinned() {
			if pinned == index {
				return item, true
			}
			pinned++
		}
	}
	return Item{}, false
}

func (s *Store) RecentText(limit int) []string {
	s.mu.Lock()
	defer s.mu.Unlock()
	var result []string
	for _, item := range s.items {
		if item.Kind == KindText && len(result) < limit {
			result = append(result, item.Text)
		}
	}
	return result
}

func (s *Store) PruneUnpinnedOlderThan(seconds int64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.pruneLocked(seconds)
}

func (s *Store) pruneLocked(seconds int64) {
	if seconds < 0 || s.db == nil {
		return
	}
	cutoff := time.Now().Unix() - seconds
	s.deleteLocked("DELETE FROM items WHERE pinned_at IS NULL AND created_at < ?", cutoff)
}

func (s *Store) Clear() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.db == nil {
		return false
	}
	return s.deleteLocked("DELETE FROM items")
}

func (s *Store) deleteLocked(statement string, args ...any) bool {
	var paths []string
	err := s.withTransactionLocked(func(tx *sql.Tx) error {
		referenced, err := imagePaths(tx)
		if err != nil {
			return err
		}
		if _, err := tx.Exec(statement, args...); err != nil {
			return err
		}
		survivors, err := imagePaths(tx)
		if err != nil {
			return err
		}
		kept := make(map[string]bool, len(survivors))
		for _, path := range survivors {
			kept[path] = true
		}
		for _, path := range referenced {
			if !kept[path] {
				paths = append(paths, path)
			}
		}
		return nil
	})
	if err != nil {
		s.lastErr = err
		return false
	}
	s.lastErr = nil
	s.removeImagesLocked(paths)
	return true
}

func imagePaths(tx *sql.Tx) ([]string, error) {
	rows, err := tx.Query("SELECT image_path FROM items WHERE image_path IS NOT NULL")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var paths []string
	for rows.Next() {
		var path string
		if err := rows.Scan(&path); err != nil {
			return nil, err
		}
		paths = append(paths, path)
	}
	return paths, rows.Err()
}

func (s *Store) removeImagesLocked(paths []string) {
	root, err := filepath.Abs(s.images)
	if err != nil {
		return
	}
	for _, path := range paths {
		absolute, err := filepath.Abs(path)
		if err != nil || filepath.Dir(absolute) != root {
			continue
		}
		info, err := os.Lstat(absolute)
		if err != nil || !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 {
			continue
		}
		var exists int
		if err := s.db.QueryRow("SELECT COUNT(*) FROM items WHERE image_path = ?", path).Scan(&exists); err != nil || exists > 0 {
			continue
		}
		os.Remove(absolute)
	}
}

func (s *Store) Delete(id string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.db == nil {
		return false
	}
	var imagePath sql.NullString
	err := s.withTransactionLocked(func(tx *sql.Tx) error {
		if err := tx.QueryRow("SELECT image_path FROM items WHERE id = ?", id).Scan(&imagePath); err != nil {
			if errors.Is(err, sql.ErrNoRows) {
				return errors.New("clipboard item no longer exists")
			}
			return err
		}
		result, err := tx.Exec("DELETE FROM items WHERE id = ?", id)
		if err != nil {
			return err
		}
		affected, err := result.RowsAffected()
		if err != nil || affected != 1 {
			return errors.New("clipboard item could not be deleted")
		}
		return nil
	})
	if err != nil {
		s.lastErr = err
		return false
	}
	s.lastErr = nil
	if imagePath.Valid {
		s.removeImagesLocked([]string{imagePath.String})
	}
	return true
}

func (s *Store) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.db == nil || s.closed {
		return nil
	}
	s.closed = true
	return s.db.Close()
}

func newID() string {
	var raw [16]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return fmt.Sprintf("%d-%d", time.Now().UnixNano(), os.Getpid())
	}
	raw[6] = (raw[6] & 0x0f) | 0x40
	raw[8] = (raw[8] & 0x3f) | 0x80
	return fmt.Sprintf("%x-%x-%x-%x-%x", raw[0:4], raw[4:6], raw[6:8], raw[8:10], raw[10:16])
}

// migrateStore copies a legacy store into a staging directory and renames it
// into place only after every database, WAL and image file has been copied.
func migrateStore(legacy, dest string) error {
	legacyDB := filepath.Join(legacy, "clipboard.sqlite3")
	if fileExists(filepath.Join(dest, "clipboard.sqlite3")) || !fileExists(legacyDB) {
		return nil
	}
	parent := filepath.Dir(dest)
	if err := os.MkdirAll(parent, 0o700); err != nil {
		return err
	}
	staging := filepath.Join(parent, "."+filepath.Base(dest)+".migrating")
	os.RemoveAll(staging)
	if err := os.Mkdir(staging, 0o700); err != nil {
		return err
	}
	cleanup := func() { os.RemoveAll(staging) }
	if err := copyFile(legacyDB, filepath.Join(staging, "clipboard.sqlite3")); err != nil {
		cleanup()
		return err
	}
	wal := filepath.Join(legacy, "clipboard.sqlite3-wal")
	if fileExists(wal) {
		if err := copyFile(wal, filepath.Join(staging, "clipboard.sqlite3-wal")); err != nil {
			cleanup()
			return err
		}
	}
	if err := copyDirectory(filepath.Join(legacy, "images"), filepath.Join(staging, "images")); err != nil {
		cleanup()
		return err
	}
	images, err := filepath.Abs(filepath.Join(dest, "images"))
	if err != nil {
		cleanup()
		return err
	}
	if err := rewriteImagePaths(filepath.Join(staging, "clipboard.sqlite3"), images); err != nil {
		cleanup()
		return err
	}
	if _, err := os.Lstat(dest); err == nil {
		cleanup()
		return errors.New("clipboard destination already exists")
	}
	if err := os.Rename(staging, dest); err != nil {
		cleanup()
		return err
	}
	return nil
}

func copyFile(source, dest string) error {
	input, err := os.Open(source)
	if err != nil {
		return err
	}
	defer input.Close()
	output, err := os.OpenFile(dest, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return err
	}
	if _, err := io.Copy(output, input); err != nil {
		output.Close()
		os.Remove(dest)
		return err
	}
	if err := output.Sync(); err != nil {
		output.Close()
		os.Remove(dest)
		return err
	}
	return output.Close()
}

func copyDirectory(source, dest string) error {
	info, err := os.Lstat(source)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return errors.New("legacy clipboard images are not a plain directory")
	}
	if err := os.MkdirAll(dest, 0o700); err != nil {
		return err
	}
	entries, err := os.ReadDir(source)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		sourcePath := filepath.Join(source, entry.Name())
		destPath := filepath.Join(dest, entry.Name())
		info, err := entry.Info()
		if err != nil {
			return err
		}
		switch {
		case info.Mode()&os.ModeSymlink != 0:
			continue
		case info.IsDir():
			if err := copyDirectory(sourcePath, destPath); err != nil {
				return err
			}
		case info.Mode().IsRegular():
			if err := copyFile(sourcePath, destPath); err != nil {
				return err
			}
		}
	}
	return nil
}

func rewriteImagePaths(dbPath, images string) error {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return err
	}
	defer db.Close()
	rows, err := db.Query("SELECT rowid, image_path FROM items WHERE image_path IS NOT NULL")
	if err != nil {
		return err
	}
	type row struct {
		id   int64
		path string
	}
	var updates []row
	for rows.Next() {
		var value row
		if err := rows.Scan(&value.id, &value.path); err != nil {
			rows.Close()
			return err
		}
		updates = append(updates, value)
	}
	rows.Close()
	for _, update := range updates {
		target := filepath.Join(images, filepath.Base(update.path))
		if fileExists(target) {
			if _, err := db.Exec("UPDATE items SET image_path = ? WHERE rowid = ?", target, update.id); err != nil {
				return err
			}
		}
	}
	return nil
}
