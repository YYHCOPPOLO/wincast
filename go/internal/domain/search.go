package domain

import (
	"math"
	"strings"
	"time"
	"unicode"

	"golang.org/x/text/unicode/norm"
)

// Fuzzy scoring floors keep exact prefixes and word starts above subsequences.
const (
	fuzzyMaximum       = 100_000
	prefixFloor        = 80_000
	wordStartFloor     = 60_000
	substringFloor     = 40_000
	subsequenceCeiling = 39_999
	// BandStride keeps a higher match band above any fuzzy score.
	BandStride = 10 * fuzzyMaximum
)

type Band int

const (
	BandExe Band = iota
	BandBundle
	BandAlternateSub
	BandDisplaySub
	BandAlternateLiteral
	BandDisplayLiteral
	BandAlias
)

// SearchFields are the names an entry can be matched by. Empty fields are
// skipped. The caller owns localization and alias storage.
type SearchFields struct {
	Alias       string
	Display     string
	Alternate   []string
	BundleID    string
	Executable  string
	SnippetWord string
}

// SearchScore returns the best banded score for query, or false when nothing
// matches. Higher is better.
func SearchScore(query string, fields SearchFields) (int, bool) {
	best := 0
	found := false
	consider := func(band Band, fuzzy int) {
		total := int(band)*BandStride + fuzzy
		if !found || total > best {
			best, found = total, true
		}
	}
	if fields.Alias != "" {
		if fuzzy, ok := fuzzyFromStart(query, fields.Alias); ok {
			consider(BandAlias, fuzzy)
		} else if fuzzy, ok := fuzzyLiteral(query, fields.Alias); ok {
			consider(BandAlternateLiteral, fuzzy)
		}
	}
	if fuzzy, ok := fuzzyScore(query, fields.Display); ok {
		if fuzzy > subsequenceCeiling {
			consider(BandDisplayLiteral, fuzzy)
		} else {
			consider(BandDisplaySub, fuzzy)
		}
	}
	if fields.SnippetWord != "" {
		if fuzzy, ok := fuzzyLiteral(query, fields.SnippetWord); ok {
			consider(BandDisplayLiteral, fuzzy)
		}
	}
	for _, name := range fields.Alternate {
		if fuzzy, ok := fuzzyScore(query, name); ok {
			if fuzzy > subsequenceCeiling {
				consider(BandAlternateLiteral, fuzzy)
			} else {
				consider(BandAlternateSub, fuzzy)
			}
		}
	}
	if fields.BundleID != "" {
		stripped := fields.BundleID
		if dot := strings.IndexByte(stripped, '.'); dot >= 0 && dot+1 < len(stripped) {
			stripped = stripped[dot+1:]
		}
		if fuzzy, ok := fuzzyLiteral(query, stripped); ok {
			consider(BandBundle, fuzzy)
		} else if fuzzy, ok := fuzzyScore(query, fields.BundleID); ok && fuzzy == fuzzyMaximum {
			consider(BandBundle, fuzzy)
		}
	}
	if fields.Executable != "" {
		if fuzzy, ok := fuzzyLiteral(query, fields.Executable); ok {
			consider(BandExe, fuzzy)
		}
	}
	return best, found
}

type normalizedText struct {
	chars     []rune
	wordStart []bool
}

func normalizeText(text string) normalizedText {
	text = norm.NFD.String(text)
	chars := make([]rune, 0, len(text))
	wordStart := make([]bool, 0, len(text))
	var previous rune
	havePrevious := false
	for _, current := range text {
		if unicode.Is(unicode.Mn, current) || isInvisibleFormat(current) {
			continue
		}
		boundary := !havePrevious || isWordBoundary(previous, current)
		first := true
		for _, lower := range strings.ToLower(string(current)) {
			chars = append(chars, lower)
			wordStart = append(wordStart, first && boundary)
			first = false
		}
		previous, havePrevious = current, true
	}
	return normalizedText{chars: chars, wordStart: wordStart}
}

func isInvisibleFormat(r rune) bool {
	switch {
	case r == 0x00AD, r == 0x061C, r == 0x06DD, r == 0x070F, r == 0x08E2,
		r == 0x180E, r == 0xFEFF, r == 0x110BD, r == 0x110CD, r == 0xE0001:
		return true
	case r >= 0x0600 && r <= 0x0605:
		return true
	case r >= 0x0890 && r <= 0x0891:
		return true
	case r >= 0x200B && r <= 0x200F:
		return true
	case r >= 0x202A && r <= 0x202E:
		return true
	case r >= 0x2060 && r <= 0x2064:
		return true
	case r >= 0x2066 && r <= 0x206F:
		return true
	case r >= 0xFFF9 && r <= 0xFFFB:
		return true
	case r >= 0x13430 && r <= 0x1343F:
		return true
	case r >= 0x1BCA0 && r <= 0x1BCA3:
		return true
	case r >= 0x1D173 && r <= 0x1D17A:
		return true
	case r >= 0xE0020 && r <= 0xE007F:
		return true
	}
	return false
}

func isWordBoundary(previous, current rune) bool {
	if !isAlphaNumeric(previous) && isAlphaNumeric(current) {
		return true
	}
	if unicode.IsLower(previous) && unicode.IsUpper(current) {
		return true
	}
	return isAlphaNumeric(previous) && isAlphaNumeric(current) && unicode.IsDigit(previous) != unicode.IsDigit(current)
}

func isAlphaNumeric(r rune) bool { return unicode.IsLetter(r) || unicode.IsNumber(r) }

func fuzzyScore(query, target string) (int, bool) {
	q, t := normalizeText(query), normalizeText(target)
	if len(q.chars) == 0 {
		return 0, false
	}
	if string(q.chars) == string(t.chars) {
		return fuzzyMaximum, true
	}
	if len(t.chars) < len(q.chars) {
		return 0, false
	}
	if hasPrefix(t.chars, q.chars) {
		return contiguousScore(prefixFloor, len(q.chars), len(t.chars), 0), true
	}
	if at, ok := findAtWordStart(q.chars, t); ok {
		return contiguousScore(wordStartFloor, len(q.chars), len(t.chars), at), true
	}
	if at, ok := findContiguous(q.chars, t.chars); ok {
		return contiguousScore(substringFloor, len(q.chars), len(t.chars), at), true
	}
	return subsequenceScore(q.chars, t)
}

func fuzzyFromStart(query, target string) (int, bool) {
	score, ok := fuzzyScore(query, target)
	if !ok || score < prefixFloor {
		return 0, false
	}
	return score, true
}

func fuzzyLiteral(query, target string) (int, bool) {
	score, ok := fuzzyScore(query, target)
	if !ok || score <= subsequenceCeiling {
		return 0, false
	}
	return score, true
}

func contiguousScore(floor, queryLength, targetLength, start int) int {
	tightness := queryLength * 9_999 / max(targetLength, 1)
	penalty := min(start, 1_999)
	score := floor + tightness - penalty
	if score < floor {
		score = floor
	}
	if score > fuzzyMaximum-1 {
		score = fuzzyMaximum - 1
	}
	return score
}

func findAtWordStart(query []rune, target normalizedText) (int, bool) {
	if len(query) == 0 || len(query) > len(target.chars) {
		return 0, false
	}
	for i := 0; i+len(query) <= len(target.chars); i++ {
		if target.wordStart[i] && equalRunes(target.chars[i:i+len(query)], query) {
			return i, true
		}
	}
	return 0, false
}

func findContiguous(query, target []rune) (int, bool) {
	if len(query) == 0 || len(query) > len(target) {
		return 0, false
	}
	for i := 0; i+len(query) <= len(target); i++ {
		if equalRunes(target[i:i+len(query)], query) {
			return i, true
		}
	}
	return 0, false
}

func subsequenceScore(query []rune, target normalizedText) (int, bool) {
	if len(query) == 0 || len(query) > len(target.chars) {
		return 0, false
	}
	bestScore := make([]int, len(query)+1)
	bestLast := make([]int, len(query)+1)
	for i := range bestScore {
		bestScore[i] = -1 << 30
	}
	bestScore[0] = 0
	for ti := range target.chars {
		for j := len(query) - 1; j >= 0; j-- {
			if query[j] != target.chars[ti] || bestScore[j] == -1<<30 {
				continue
			}
			score := bestScore[j] + 16
			if target.wordStart[ti] {
				score += 12
			}
			if j > 0 {
				previous := bestLast[j]
				if ti == previous+1 {
					score += 8
				} else if ti > previous {
					score -= min(ti-previous-1, 8)
				}
			}
			if score > bestScore[j+1] {
				bestScore[j+1], bestLast[j+1] = score, ti
			}
		}
	}
	if bestScore[len(query)] == -1<<30 {
		return 0, false
	}
	score := bestScore[len(query)]
	if score < 1 {
		score = 1
	}
	if score > subsequenceCeiling {
		score = subsequenceCeiling
	}
	return score, true
}

func hasPrefix(text, prefix []rune) bool {
	return len(text) >= len(prefix) && equalRunes(text[:len(prefix)], prefix)
}

func equalRunes(a, b []rune) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

// RankingBoost is a frecency score computed by a store outside the domain.
// It occupies the space below the first search band.
const maximumRankingBoost = 4_500

func ClampRankingBoost(value int) int {
	if value < 0 {
		return 0
	}
	if value > maximumRankingBoost {
		return maximumRankingBoost
	}
	return value
}

// RankingRecord is one stored visit. Count increments for every recorded use.
type RankingRecord struct {
	ItemKey  string `json:"itemKey"`
	Query    string `json:"query"`
	Count    uint32 `json:"count"`
	LastUsed int64  `json:"lastUsed"`
}

// RankingScore blends frequency and recency exactly as launcher_ranking.rs.
func RankingScore(record RankingRecord, now time.Time) int {
	ageSeconds := now.Unix() - record.LastUsed
	if ageSeconds < 0 {
		ageSeconds = 0
	}
	ageDays := float64(ageSeconds) / 86_400
	frequency := 600.0 * math.Log2(1+float64(record.Count))
	if frequency > 3_000 {
		frequency = 3_000
	}
	recency := 1_500 * math.Exp(-ageDays/14.0)
	return ClampRankingBoost(int(frequency + recency + 0.5))
}
