// Package filesearch coordinates Windows Search with the bounded directory
// walk. The UI asks for a query; stale workers are discarded by the caller's
// revision, and every disk read checks the request context.
package filesearch

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"

	domain "github.com/YYHCOPPOLO/wincast/go/internal/domain/filesearch"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

const (
	Debounce       = 120 * time.Millisecond
	MaxWalkDepth   = 12
	maxWalkEntries = 8_000
)

// State mirrors the file-search screen's four visible states.
type State uint8

const (
	Idle State = iota
	Searching
	Ready
	Failed
)

// Session serializes search requests and owns cancellation. Search itself
// must not touch the UI; done runs on the worker goroutine.
type Session struct {
	mu       sync.Mutex
	policy   domain.Policy
	revision uint64
	cancel   context.CancelFunc
	state    State
	results  []domain.Hit
	query    string
}

func New() *Session {
	return &Session{policy: domain.NewPolicy(domain.DefaultScopes(), nil, homeDirectory())}
}

func homeDirectory() string {
	if home, err := os.UserHomeDir(); err == nil {
		return home
	}
	return ""
}

func (s *Session) Apply(scopes, ignores []string) {
	policy := domain.NewPolicy(scopes, ignores, homeDirectory())
	s.mu.Lock()
	s.policy = policy
	s.mu.Unlock()
}

func (s *Session) Policy() domain.Policy {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.policy
}

func (s *Session) State() (State, []domain.Hit, string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.state, append([]domain.Hit(nil), s.results...), s.query
}

// Cancel stops the active request without clearing the last visible rows.
func (s *Session) Cancel() {
	s.mu.Lock()
	if s.cancel != nil {
		s.cancel()
		s.cancel = nil
	}
	s.mu.Unlock()
}

// Search invalidates the previous request and, after the debounce, runs the
// index query or the walk. done receives the request revision so the caller
// can discard a superseded result even if a worker already finished.
func (s *Session) Search(parent context.Context, query string, done func(revision uint64, state State, hits []domain.Hit, err error)) uint64 {
	query = strings.TrimSpace(query)
	s.mu.Lock()
	if s.cancel != nil {
		s.cancel()
	}
	ctx, cancel := context.WithCancel(parent)
	s.cancel = cancel
	s.revision++
	revision := s.revision
	s.query = query
	s.mu.Unlock()
	if query == "" {
		s.mu.Lock()
		s.state = Idle
		s.results = nil
		s.mu.Unlock()
		if done != nil {
			done(revision, Idle, nil, nil)
		}
		return revision
	}
	s.mu.Lock()
	s.state = Searching
	s.mu.Unlock()
	if done != nil {
		done(revision, Searching, nil, nil)
	}
	policy := s.Policy()
	go func() {
		timer := time.NewTimer(Debounce)
		defer timer.Stop()
		select {
		case <-ctx.Done():
			return
		case <-timer.C:
		}
		hits, err := Search(ctx, query, policy)
		if ctx.Err() != nil {
			return
		}
		state := Ready
		if err != nil && !errors.Is(err, context.Canceled) {
			state = Failed
		}
		s.mu.Lock()
		if revision != s.revision {
			s.mu.Unlock()
			return
		}
		s.state = state
		s.results = hits
		s.cancel = nil
		s.mu.Unlock()
		if done != nil {
			done(revision, state, hits, err)
		}
	}()
	return revision
}

// Search runs the index first and falls back to the capped walk when the
// index is unavailable or returns no usable catalog rows.
func Search(ctx context.Context, query string, policy domain.Policy) ([]domain.Hit, error) {
	return search(ctx, query, policy, windows.SearchIndexPaths)
}

type indexFunc func(context.Context, string) ([]string, error)

func search(ctx context.Context, query string, policy domain.Policy, index indexFunc) ([]domain.Hit, error) {
	if len(domain.Tokens(query)) == 0 {
		return nil, nil
	}
	if index != nil {
		hits, err := indexSearch(ctx, query, policy, index)
		if err == nil && len(hits) > 0 {
			return domain.Rank(hits, query, policy.Ignore), nil
		}
	}
	return domain.Rank(walk(ctx, query, policy), query, policy.Ignore), nil
}

func indexSearch(ctx context.Context, query string, policy domain.Policy, index indexFunc) ([]domain.Hit, error) {
	scopes := resolvedDirectories(policy)
	if len(scopes) == 0 && !policy.IncludesHome {
		return nil, nil
	}
	where, ok := domain.LikeClauses(query)
	if !ok {
		return nil, nil
	}
	scopeSQL := make([]string, 0, len(scopes))
	for _, dir := range scopes {
		native := strings.ReplaceAll(dir, "/", `\`)
		scopeSQL = append(scopeSQL, "SCOPE='file:"+strings.ReplaceAll(native, "'", "''")+"'")
	}
	if len(scopeSQL) == 0 {
		return nil, errors.New("file search: no scope")
	}
	extras := ""
	for _, glob := range policy.Ignore.SearchNameExclusions() {
		extras += " AND NOT System.FileName LIKE '" + strings.ReplaceAll(glob, "'", "''") + "'"
	}
	sql := "SELECT TOP 1000 System.ItemPathDisplay FROM SystemIndex WHERE (" +
		strings.Join(scopeSQL, " OR ") + ") AND (" + where + ")" + extras
	paths, err := index(ctx, sql)
	if err != nil {
		return nil, err
	}
	hits := make([]domain.Hit, 0, len(paths)+8)
	seen := make(map[string]bool, len(paths)+8)
	if policy.IncludesHome {
		for _, hit := range homeRootMatches(query, policy) {
			if !seen[hit.Path] {
				seen[hit.Path] = true
				hits = append(hits, hit)
			}
		}
	}
	for _, path := range paths {
		if len(hits) >= domain.CandidateLimit {
			break
		}
		if domain.IsExcludedPath(path, policy.Ignore) {
			continue
		}
		info, err := os.Stat(path)
		isDir := err == nil && info.IsDir()
		hit := domain.HitFromPath(path, isDir, policy.Home)
		if seen[hit.Path] {
			continue
		}
		seen[hit.Path] = true
		hits = append(hits, hit)
	}
	return hits, nil
}

// resolvedDirectories expands the home scope into its visible children, as
// the oracle does, and never searches AppData by itself.
func resolvedDirectories(policy domain.Policy) []string {
	dirs := make([]string, 0, len(policy.DirectRoots)+8)
	seen := make(map[string]bool, len(policy.DirectRoots)+8)
	add := func(path string) {
		key := strings.ToLower(path)
		if path == "" || seen[key] {
			return
		}
		seen[key] = true
		dirs = append(dirs, path)
	}
	for _, root := range policy.DirectRoots {
		add(root)
	}
	if policy.IncludesHome {
		for _, entry := range listDir(policy.Home) {
			if entry.IsDir && !entry.Reparse && domain.KeepHomeChild(entry.Name, entry.Hidden) {
				add(entry.Path)
			}
		}
	}
	return dirs
}

func homeRootMatches(query string, policy domain.Policy) []domain.Hit {
	if !policy.IncludesHome {
		return nil
	}
	out := make([]domain.Hit, 0, 16)
	for _, entry := range listDir(policy.Home) {
		if !domain.KeepHomeChild(entry.Name, entry.Hidden) {
			continue
		}
		if !domain.MatchesFilename(entry.Name, query) {
			continue
		}
		path := strings.ReplaceAll(entry.Path, `\`, "/")
		if domain.IsExcludedPath(path, policy.Ignore) {
			continue
		}
		out = append(out, domain.HitFromPath(path, entry.IsDir, policy.Home))
	}
	return out
}

type dirent struct {
	Path    string
	Name    string
	IsDir   bool
	Reparse bool
	Hidden  bool
}

func listDir(dir string) []dirent {
	if dir == "" {
		return nil
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	out := make([]dirent, 0, len(entries))
	for _, entry := range entries {
		name := entry.Name()
		if name == "." || name == ".." {
			continue
		}
		info, err := entry.Info()
		if err != nil {
			continue
		}
		attrs := fileAttributes(info)
		hidden := strings.HasPrefix(name, ".") || attrs&0x2 != 0
		reparse := attrs&0x400 != 0 || info.Mode()&os.ModeSymlink != 0
		out = append(out, dirent{
			Path:    filepath.Join(dir, name),
			Name:    name,
			IsDir:   entry.IsDir(),
			Reparse: reparse,
			Hidden:  hidden,
		})
	}
	return out
}

// walk runs the bounded fallback. Home children are searched individually;
// direct roots are searched recursively. Hidden and reparse directories are
// never descended into.
func walk(ctx context.Context, query string, policy domain.Policy) []domain.Hit {
	hits := walkHomeMatches(query, policy)
	seen := make(map[string]bool, len(hits)+64)
	for _, hit := range hits {
		seen[hit.Path] = true
	}
	type item struct {
		path  string
		depth int
	}
	stack := make([]item, 0, len(policy.DirectRoots)+8)
	for _, root := range resolvedDirectories(policy) {
		stack = append(stack, item{path: root, depth: 0})
	}
	visited := 0
	for len(stack) > 0 && len(hits) < domain.CandidateLimit {
		if err := ctx.Err(); err != nil {
			break
		}
		current := stack[len(stack)-1]
		stack = stack[:len(stack)-1]
		if current.depth > MaxWalkDepth {
			continue
		}
		for _, entry := range listDir(current.path) {
			visited++
			if visited > maxWalkEntries {
				return hits
			}
			path := strings.ReplaceAll(entry.Path, `\`, "/")
			if domain.IsExcludedPath(path, policy.Ignore) {
				continue
			}
			if entry.IsDir && !entry.Reparse {
				stack = append(stack, item{path: entry.Path, depth: current.depth + 1})
			}
			if !domain.MatchesFilename(entry.Name, query) {
				continue
			}
			if seen[path] {
				continue
			}
			seen[path] = true
			hits = append(hits, domain.HitFromPath(path, entry.IsDir, policy.Home))
			if len(hits) >= domain.CandidateLimit {
				return hits
			}
		}
	}
	return hits
}

func walkHomeMatches(query string, policy domain.Policy) []domain.Hit {
	if !policy.IncludesHome {
		return nil
	}
	out := make([]domain.Hit, 0, 16)
	for _, entry := range listDir(policy.Home) {
		if !domain.KeepHomeChild(entry.Name, entry.Hidden) || !domain.MatchesFilename(entry.Name, query) {
			continue
		}
		path := strings.ReplaceAll(entry.Path, `\`, "/")
		if domain.IsExcludedPath(path, policy.Ignore) {
			continue
		}
		out = append(out, domain.HitFromPath(path, entry.IsDir, policy.Home))
	}
	return out
}

func fileAttributes(info os.FileInfo) uint32 {
	if data, ok := info.Sys().(*syscall.Win32FileAttributeData); ok {
		return data.FileAttributes
	}
	return 0
}
