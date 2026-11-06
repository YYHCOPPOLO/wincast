// Package launcher owns the application catalog, frecency, favorites and
// per-entry aliases. It keeps display state out of the UI and reads/writes
// only its own launcher directory.
package launcher

import (
	"encoding/json"
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"time"
	"unicode"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"golang.org/x/text/unicode/norm"
)

const (
	maximumRecords = 1_000
	prefixLimit    = 64
)

type Preferences struct {
	dir       string
	Aliases   map[string]string
	Favorites []string
	Hidden    map[string]bool
	Ranking   map[string]map[string]domain.RankingRecord
}

func LoadPreferences(dir string) (*Preferences, error) {
	p := &Preferences{
		dir: dir, Aliases: make(map[string]string), Hidden: make(map[string]bool),
		Ranking: make(map[string]map[string]domain.RankingRecord),
	}
	var firstErr error
	if err := loadJSON(filepath.Join(dir, "favorites.json"), &p.Favorites); err != nil {
		firstErr = err
	}
	p.Favorites = sanitizeIDs(p.Favorites)
	if err := loadJSON(filepath.Join(dir, "aliases.json"), &p.Aliases); err != nil && firstErr == nil {
		firstErr = err
	}
	for id, alias := range p.Aliases {
		if id == "" || strings.TrimSpace(alias) == "" {
			delete(p.Aliases, id)
		}
	}
	if err := loadJSON(filepath.Join(dir, "hidden.json"), &p.Hidden); err != nil && firstErr == nil {
		firstErr = err
	}
	var records []domain.RankingRecord
	if err := loadJSON(filepath.Join(dir, "ranking.json"), &records); err != nil && firstErr == nil {
		firstErr = err
	}
	for _, record := range records {
		if record.ItemKey == "" || record.Query == "" || record.Count == 0 {
			continue
		}
		if p.Ranking[record.Query] == nil {
			p.Ranking[record.Query] = make(map[string]domain.RankingRecord)
		}
		p.Ranking[record.Query][record.ItemKey] = record
	}
	return p, firstErr
}

func loadJSON(path string, target any) error {
	data, err := os.ReadFile(path)
	if errors.Is(err, fs.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if err := json.Unmarshal(data, target); err != nil {
		return err
	}
	return nil
}

func sanitizeIDs(ids []string) []string {
	clean := make([]string, 0, len(ids))
	for _, id := range ids {
		if id == "" || slices.Contains(clean, id) {
			continue
		}
		clean = append(clean, id)
	}
	return clean
}

func (p *Preferences) Alias(id string) string { return p.Aliases[id] }

func (p *Preferences) SetAlias(id, alias string) error {
	if id == "" {
		return errors.New("empty application id")
	}
	if strings.TrimSpace(alias) == "" {
		delete(p.Aliases, id)
	} else {
		p.Aliases[id] = alias
	}
	return p.saveAliases()
}

func (p *Preferences) IsHidden(id string) bool { return p.Hidden[id] }

func (p *Preferences) SetHidden(id string, hidden bool) error {
	if id == "" {
		return errors.New("empty application id")
	}
	if hidden {
		p.Hidden[id] = true
	} else {
		delete(p.Hidden, id)
	}
	return p.saveHidden()
}

// ToggleFavorite appends the id, or removes it without reordering the rest.
func (p *Preferences) ToggleFavorite(id string) error {
	if id == "" {
		return errors.New("empty application id")
	}
	if index := slices.Index(p.Favorites, id); index >= 0 {
		p.Favorites = slices.Delete(p.Favorites, index, index+1)
	} else {
		p.Favorites = append(p.Favorites, id)
	}
	return p.saveFavorites()
}

func (p *Preferences) IsFavorite(id string) bool { return slices.Contains(p.Favorites, id) }

// FavoriteSlot maps Ctrl+1..9 to indices 0..8 and Ctrl+0 to index 9.
func (p *Preferences) FavoriteSlot(digit byte) string {
	var index int
	switch digit {
	case '1', '2', '3', '4', '5', '6', '7', '8', '9':
		index = int(digit - '1')
	case '0':
		index = 9
	default:
		return ""
	}
	if index < len(p.Favorites) {
		return p.Favorites[index]
	}
	return ""
}

// FavoriteOrder returns stored favorite ids for empty-query ordering.
func (p *Preferences) FavoriteOrder() []string { return p.Favorites }

// Record trains every normalized prefix of the query. Empty queries and
// category listings are not recorded by callers.
func (p *Preferences) Record(query, id string, now time.Time) {
	query = normalizeQuery(query)
	if query == "" || id == "" {
		return
	}
	prefix := ""
	count := 0
	for _, r := range []rune(query) {
		if count >= prefixLimit {
			break
		}
		prefix += string(r)
		count++
		byItem := p.Ranking[prefix]
		if byItem == nil {
			byItem = make(map[string]domain.RankingRecord)
			p.Ranking[prefix] = byItem
		}
		record := byItem[id]
		record.ItemKey, record.Query = id, prefix
		record.Count++
		record.LastUsed = now.Unix()
		byItem[id] = record
	}
	p.evict()
}

func (p *Preferences) Boost(query, id string, now time.Time) int {
	byItem := p.Ranking[normalizeQuery(query)]
	record, ok := byItem[id]
	if !ok {
		return 0
	}
	return domain.RankingScore(record, now)
}

func (p *Preferences) ResetRanking(id string) {
	for query, byItem := range p.Ranking {
		delete(byItem, id)
		if len(byItem) == 0 {
			delete(p.Ranking, query)
		}
	}
}

func (p *Preferences) evict() {
	total := 0
	for _, byItem := range p.Ranking {
		total += len(byItem)
	}
	if total <= maximumRecords {
		return
	}
	var all []domain.RankingRecord
	for _, byItem := range p.Ranking {
		for _, record := range byItem {
			all = append(all, record)
		}
	}
	slices.SortFunc(all, func(a, b domain.RankingRecord) int {
		if a.Count != b.Count {
			return int(b.Count) - int(a.Count)
		}
		return int(b.LastUsed - a.LastUsed)
	})
	all = all[:maximumRecords]
	p.Ranking = make(map[string]map[string]domain.RankingRecord)
	for _, record := range all {
		if p.Ranking[record.Query] == nil {
			p.Ranking[record.Query] = make(map[string]domain.RankingRecord)
		}
		p.Ranking[record.Query][record.ItemKey] = record
	}
}

func normalizeQuery(query string) string {
	query = strings.TrimSpace(query)
	query = norm.NFD.String(query)
	var builder strings.Builder
	for _, r := range query {
		if unicode.Is(unicode.Mn, r) {
			continue
		}
		builder.WriteString(strings.ToLower(string(r)))
	}
	return builder.String()
}

func (p *Preferences) saveFavorites() error {
	return saveJSON(filepath.Join(p.dir, "favorites.json"), p.Favorites)
}

func (p *Preferences) saveAliases() error {
	return saveJSON(filepath.Join(p.dir, "aliases.json"), p.Aliases)
}

func (p *Preferences) saveHidden() error {
	return saveJSON(filepath.Join(p.dir, "hidden.json"), p.Hidden)
}

// RankingBytes returns the flat records sorted for a stable file.
func (p *Preferences) RankingBytes() ([]byte, error) {
	var records []domain.RankingRecord
	for _, byItem := range p.Ranking {
		for _, record := range byItem {
			records = append(records, record)
		}
	}
	slices.SortFunc(records, func(a, b domain.RankingRecord) int {
		if a.Query != b.Query {
			return strings.Compare(a.Query, b.Query)
		}
		return strings.Compare(a.ItemKey, b.ItemKey)
	})
	return json.MarshalIndent(records, "", "  ")
}

// SaveRanking writes the frecency table atomically.
func (p *Preferences) SaveRanking() error {
	data, err := p.RankingBytes()
	if err != nil {
		return err
	}
	return storage.WriteFileAtomic(filepath.Join(p.dir, "ranking.json"), append(data, '\n'), 0o600)
}

func saveJSON(path string, value any) error {
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	return storage.WriteFileAtomic(path, append(data, '\n'), 0o600)
}
