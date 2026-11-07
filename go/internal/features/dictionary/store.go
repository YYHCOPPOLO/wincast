package dictionary

import (
	"context"
	"database/sql"
	"errors"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

const columns = "word, phonetic, definition, translation, pos, collins, oxford, tag, bnc, frq, exchange, detail, audio"
const order = "ORDER BY oxford DESC, collins DESC, (frq <= 0), frq ASC, (bnc <= 0), bnc ASC, length(word)"

type Entry struct {
	Word, Phonetic, Definition, Translation, POS string
	Collins, Oxford                              int64
	Tag                                          string
	BNC, Frequency                               int64
	Exchange, Detail, Audio                      string
}

type Store struct{ db *sql.DB }

func Open(ctx context.Context, path string) (*Store, error) {
	database, err := storage.OpenSQLite(ctx, path, true)
	if err != nil {
		return nil, err
	}
	statement, err := database.PrepareContext(ctx, "SELECT "+columns+" FROM words WHERE word=? COLLATE NOCASE LIMIT 1")
	if err != nil {
		database.Close()
		return nil, err
	}
	statement.Close()
	return &Store{db: database}, nil
}

func (s *Store) Close() error { return s.db.Close() }

func (s *Store) Lookup(ctx context.Context, query string) (*Entry, error) {
	query = strings.TrimSpace(query)
	if query == "" {
		return nil, nil
	}
	entry, err := s.first(ctx, "SELECT "+columns+" FROM words WHERE word=? COLLATE NOCASE LIMIT 1", query)
	if err != nil || entry != nil {
		return entry, err
	}
	key := searchKey(query)
	if key == "" {
		return nil, nil
	}
	return s.first(ctx, "SELECT "+columns+" FROM words WHERE sw=? "+order+" LIMIT 1", key)
}

// Suggestions returns prefix neighbours for a miss, best first. A query past
// the last matching word backs off one character at a time.
func (s *Store) Suggestions(ctx context.Context, query string, limit int) ([]string, error) {
	prefix := searchKey(strings.TrimSpace(query))
	if prefix == "" || limit <= 0 {
		return nil, nil
	}
	for {
		words, err := s.prefixWords(ctx, prefix, strings.TrimSpace(query), limit)
		if err != nil {
			return nil, err
		}
		if len(words) > 0 || len(prefix) == 1 {
			return words, nil
		}
		prefix = prefix[:len(prefix)-1]
		if prefix == "" {
			return nil, nil
		}
	}
}

func (s *Store) prefixWords(ctx context.Context, prefix, query string, limit int) ([]string, error) {
	end := prefix + "\U0010FFFF"
	statement := "SELECT word FROM words WHERE sw >= ? AND sw < ? AND word <> ? COLLATE NOCASE " + order + " LIMIT ?"
	rows, err := s.db.QueryContext(ctx, statement, prefix, end, query, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var words []string
	for rows.Next() {
		var word string
		if err := rows.Scan(&word); err != nil {
			return nil, err
		}
		words = append(words, word)
	}
	return words, rows.Err()
}

func (s *Store) first(ctx context.Context, statement, query string) (*Entry, error) {
	var entry Entry
	err := s.db.QueryRowContext(ctx, statement, query).Scan(&entry.Word, &entry.Phonetic, &entry.Definition, &entry.Translation,
		&entry.POS, &entry.Collins, &entry.Oxford, &entry.Tag, &entry.BNC, &entry.Frequency, &entry.Exchange, &entry.Detail, &entry.Audio)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &entry, nil
}

func searchKey(text string) string {
	var key strings.Builder
	for _, r := range text {
		if r >= 'A' && r <= 'Z' {
			r += 'a' - 'A'
		}
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') {
			key.WriteRune(r)
		}
	}
	return key.String()
}
