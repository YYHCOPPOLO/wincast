// Package filesearch holds the pure file-search rules: query tokens, ignore
// patterns, scope expansion, hit identity and ranking. It never touches disk.
package filesearch

import (
	"cmp"
	"slices"
	"strings"
	"unicode"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"golang.org/x/text/unicode/norm"
)

const (
	CandidateLimit = 1_000
	ResultLimit    = 200
)

// Hit is one file or directory row. Path uses forward slashes; Parent is
// abbreviated with "~" when it is inside the home directory.
type Hit struct {
	Path        string
	Name        string
	Parent      string
	IsDirectory bool
}

func (h Hit) ID() string { return h.Path }

func HitFromPath(path string, isDirectory bool, home string) Hit {
	path = NormalizeSlashes(path)
	return Hit{
		Path:        path,
		Name:        Filename(path),
		Parent:      Abbreviate(ParentOf(path), home),
		IsDirectory: isDirectory,
	}
}

func Tokens(query string) []string {
	return strings.Fields(query)
}

func CapCandidates(n int) int { return min(n, CandidateLimit) }
func CapRows(n int) int       { return min(n, ResultLimit) }

// MatchesFilename requires every query token to occur in the filename,
// case- and diacritic-insensitively.
func MatchesFilename(filename, query string) bool {
	folded := Fold(filename)
	for _, term := range Tokens(query) {
		if !strings.Contains(folded, Fold(term)) {
			return false
		}
	}
	return true
}

// IsExcludedPath treats hidden components and application bundles as
// structural exclusions, then applies the user's ignore patterns.
func IsExcludedPath(path string, ignore IgnoreList) bool {
	for _, component := range Components(path) {
		if (strings.HasPrefix(component, ".") && component != "." && component != "..") ||
			strings.HasSuffix(strings.ToLower(component), ".app") {
			return true
		}
	}
	return ignore.Drops(path)
}

// Rank keeps the Rust ordering: the whole query's fuzzy score first, then the
// sum of per-token scores, then filename and path alphabetically.
func Rank(hits []Hit, query string, ignore IgnoreList) []Hit {
	terms := Tokens(query)
	if len(terms) == 0 {
		return nil
	}
	whole := strings.TrimSpace(query)
	type scored struct {
		hit   Hit
		whole int
		total int
		ok    bool
	}
	filtered := make([]scored, 0, len(hits))
	for _, hit := range hits {
		if IsExcludedPath(hit.Path, ignore) {
			continue
		}
		value, ok := domain.SearchScore(whole, domain.SearchFields{Display: hit.Name})
		total := 0
		for _, term := range terms {
			if termScore, matched := domain.SearchScore(term, domain.SearchFields{Display: hit.Name}); matched {
				total += termScore
			}
		}
		filtered = append(filtered, scored{hit: hit, whole: value, total: total, ok: ok})
	}
	slices.SortFunc(filtered, func(a, b scored) int {
		if a.ok != b.ok {
			if a.ok {
				return -1
			}
			return 1
		}
		if a.whole != b.whole {
			return cmp.Compare(b.whole, a.whole)
		}
		if a.total != b.total {
			return cmp.Compare(b.total, a.total)
		}
		if byName := strings.Compare(strings.ToLower(a.hit.Name), strings.ToLower(b.hit.Name)); byName != 0 {
			return byName
		}
		return strings.Compare(strings.ToLower(a.hit.Path), strings.ToLower(b.hit.Path))
	})
	if len(filtered) > ResultLimit {
		filtered = filtered[:ResultLimit]
	}
	result := make([]Hit, len(filtered))
	for i, item := range filtered {
		result[i] = item.hit
	}
	return result
}

// LikeClauses builds the Windows Search WHERE fragment for every token.
func LikeClauses(query string) (string, bool) {
	terms := Tokens(query)
	if len(terms) == 0 {
		return "", false
	}
	clauses := make([]string, 0, len(terms))
	for _, term := range terms {
		clauses = append(clauses, "System.FileName LIKE '%"+escapeLike(term)+"%'")
	}
	return strings.Join(clauses, " AND "), true
}

func escapeLike(term string) string {
	var out strings.Builder
	for _, r := range term {
		switch r {
		case '\'':
			out.WriteString("''")
		case '%':
			out.WriteString("[%]")
		case '_':
			out.WriteString("[_]")
		case '[':
			out.WriteString("[[]")
		default:
			out.WriteRune(r)
		}
	}
	return out.String()
}

func Fold(text string) string {
	var out strings.Builder
	out.Grow(len(text))
	for _, r := range norm.NFD.String(text) {
		if unicode.Is(unicode.Mn, r) {
			continue
		}
		out.WriteRune(unicode.ToLower(r))
	}
	return out.String()
}

func NormalizeSlashes(path string) string {
	return strings.ReplaceAll(path, `\`, "/")
}

func Components(path string) []string {
	parts := strings.FieldsFunc(path, func(r rune) bool { return r == '/' || r == '\\' })
	out := make([]string, 0, len(parts))
	for _, part := range parts {
		if part == "." || part == ".." || part == "" {
			continue
		}
		out = append(out, part)
	}
	return out
}

func Filename(path string) string {
	parts := Components(path)
	if len(parts) == 0 {
		return path
	}
	return parts[len(parts)-1]
}

func ParentOf(path string) string {
	normalized := NormalizeSlashes(path)
	index := strings.LastIndex(normalized, "/")
	switch index {
	case -1:
		return ""
	case 0:
		return "/"
	default:
		return normalized[:index]
	}
}

func Abbreviate(path, home string) string {
	path = NormalizeSlashes(path)
	home = strings.TrimSuffix(NormalizeSlashes(home), "/")
	if path == home {
		return "~"
	}
	if rest, ok := strings.CutPrefix(path, home+"/"); ok {
		return "~/" + rest
	}
	return path
}

func ExpandScope(scope, home string) string {
	home = strings.TrimSuffix(NormalizeSlashes(home), "/")
	if scope == "~" {
		return home
	}
	if rest, ok := strings.CutPrefix(scope, "~/"); ok {
		return home + "/" + rest
	}
	if rest, ok := strings.CutPrefix(scope, `~\`); ok {
		return home + "/" + rest
	}
	return NormalizeSlashes(scope)
}

// Policy is the resolved answer to what the configured scopes mean.
type Policy struct {
	Home         string
	DirectRoots  []string
	IncludesHome bool
	Ignore       IgnoreList
}

func NewPolicy(scopes, ignorePatterns []string, home string) Policy {
	home = strings.TrimSuffix(NormalizeSlashes(home), "/")
	roots := uniqueRoots(scopes, home)
	direct := make([]string, 0, len(roots))
	includesHome := false
	for _, root := range roots {
		if root == home {
			includesHome = true
			continue
		}
		direct = append(direct, root)
	}
	return Policy{Home: home, DirectRoots: direct, IncludesHome: includesHome, Ignore: NewIgnoreList(ignorePatterns)}
}

func DefaultScopes() []string { return []string{"~"} }

func uniqueRoots(scopes []string, home string) []string {
	seen := make(map[string]bool, len(scopes))
	out := make([]string, 0, len(scopes))
	for _, scope := range scopes {
		expanded := ExpandScope(scope, home)
		if expanded == "" || seen[expanded] {
			continue
		}
		seen[expanded] = true
		out = append(out, expanded)
	}
	return out
}

// NormalizeScopes rewrites scopes as "~" relative paths for persistence.
func NormalizeScopes(scopes []string, home string) []string {
	seen := make(map[string]bool, len(scopes))
	out := make([]string, 0, len(scopes))
	for _, root := range uniqueRoots(scopes, home) {
		value := Abbreviate(root, home)
		if seen[value] {
			continue
		}
		seen[value] = true
		out = append(out, value)
	}
	return out
}

// KeepHomeChild limits the home scope to visible user folders and never
// descends into application data.
func KeepHomeChild(name string, hidden bool) bool {
	if hidden {
		return false
	}
	lower := strings.ToLower(Filename(name))
	return lower != "appdata" && lower != "library" && lower != "application data"
}

// IgnoreList is a gitignore-flavoured set of excludes.
type IgnoreList struct {
	literalNames map[string]bool
	nameGlobs    []string
	pathGlobs    []string
}

var DefaultIgnorePatterns = []string{"node_modules", "DerivedData", "build", "dist", "target", "Pods"}

func DefaultIgnoreList() IgnoreList { return NewIgnoreList(nil) }

func NewIgnoreList(patterns []string) IgnoreList {
	ignore := IgnoreList{literalNames: make(map[string]bool)}
	all := make([]string, 0, len(DefaultIgnorePatterns)+len(patterns))
	all = append(all, DefaultIgnorePatterns...)
	all = append(all, patterns...)
	for _, pattern := range all {
		trimmed := strings.TrimSpace(pattern)
		if trimmed == "" || strings.ContainsRune(trimmed, 0) {
			continue
		}
		if strings.ContainsAny(trimmed, `/\`) {
			ignore.pathGlobs = append(ignore.pathGlobs, strings.ReplaceAll(trimmed, `\`, "/"))
			continue
		}
		if strings.ContainsAny(trimmed, "*?[") {
			ignore.nameGlobs = append(ignore.nameGlobs, trimmed)
			continue
		}
		ignore.literalNames[strings.ToLower(trimmed)] = true
	}
	return ignore
}

func (l IgnoreList) Drops(path string) bool {
	for _, component := range Components(path) {
		if l.literalNames[strings.ToLower(component)] {
			return true
		}
		for _, glob := range l.nameGlobs {
			if GlobMatch(glob, component) {
				return true
			}
		}
	}
	normalized := NormalizeSlashes(path)
	for _, glob := range l.pathGlobs {
		if GlobMatch(glob, normalized) {
			return true
		}
	}
	return false
}

// SearchNameExclusions returns name globs Windows Search can evaluate itself
// (only '*' remains meaningful there).
func (l IgnoreList) SearchNameExclusions() []string {
	out := make([]string, 0, len(l.nameGlobs))
	for _, glob := range l.nameGlobs {
		if strings.ContainsAny(glob, `?["\`) {
			continue
		}
		out = append(out, glob)
	}
	return out
}

// GlobMatch is a case-insensitive glob where '*' spans path separators.
func GlobMatch(pattern, candidate string) bool {
	pat := []rune(pattern)
	text := []rune(candidate)
	pi, ti := 0, 0
	starP, starT := -1, 0
	for ti < len(text) {
		if pi < len(pat) && pat[pi] == '[' {
			ok, next := matchClass(pat, pi, text[ti])
			if ok {
				pi, ti = next, ti+1
				continue
			}
			if starP >= 0 {
				pi = starP + 1
				starT++
				ti = starT
				continue
			}
			return false
		}
		if pi < len(pat) && (pat[pi] == '?' || equalFoldRune(pat[pi], text[ti])) {
			pi, ti = pi+1, ti+1
			continue
		}
		if pi < len(pat) && pat[pi] == '*' {
			starP, starT = pi, ti
			pi++
			continue
		}
		if starP >= 0 {
			pi = starP + 1
			starT++
			ti = starT
			continue
		}
		return false
	}
	for pi < len(pat) && pat[pi] == '*' {
		pi++
	}
	return pi == len(pat)
}

func matchClass(pat []rune, pi int, ch rune) (bool, int) {
	if pi >= len(pat) || pat[pi] != '[' {
		return false, pi
	}
	pi++
	negate := pi < len(pat) && (pat[pi] == '!' || pat[pi] == '^')
	if negate {
		pi++
	}
	matched, saw := false, false
	for pi < len(pat) && pat[pi] != ']' {
		saw = true
		if pi+2 < len(pat) && pat[pi+1] == '-' && pat[pi+2] != ']' {
			if inRangeFold(ch, pat[pi], pat[pi+2]) {
				matched = true
			}
			pi += 3
			continue
		}
		if equalFoldRune(pat[pi], ch) {
			matched = true
		}
		pi++
	}
	if pi < len(pat) && pat[pi] == ']' {
		pi++
	}
	if !saw {
		return false, pi
	}
	if negate {
		return !matched, pi
	}
	return matched, pi
}

func equalFoldRune(a, b rune) bool {
	return unicode.ToLower(a) == unicode.ToLower(b)
}

func inRangeFold(ch, start, end rune) bool {
	c := unicode.ToLower(ch)
	a := unicode.ToLower(start)
	b := unicode.ToLower(end)
	return (a <= c && c <= b) || (b <= c && c <= a)
}
