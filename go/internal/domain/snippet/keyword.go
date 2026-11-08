package snippet

import "strings"

const (
	// KeywordTimeoutMS clears the buffer after this much inactivity.
	KeywordTimeoutMS int64 = 15000
	// KeywordMaximumLength caps the buffer, in characters.
	KeywordMaximumLength = 256
)

// MatchSuffix returns the longest configured keyword that ends the buffer.
func MatchSuffix(buffer string, keywords []string) (string, bool) {
	normalized := strings.ToLower(buffer)
	best, found := "", false
	for _, keyword := range keywords {
		trimmed := strings.TrimSpace(keyword)
		if trimmed == "" || len([]rune(trimmed)) > KeywordMaximumLength {
			continue
		}
		needle := strings.ToLower(trimmed)
		if strings.HasSuffix(normalized, needle) && (!found || len([]rune(needle)) > len([]rune(best))) {
			best, found = trimmed, true
		}
	}
	return best, found
}

// KeywordBuffer accumulates recently typed characters. It never stores or
// reports a key stream: callers only learn the matched keyword.
type KeywordBuffer struct {
	buffer   []rune
	lastMS   int64
	hasLast  bool
	keywords []string
}

func NewKeywordBuffer(keywords []string) *KeywordBuffer {
	buffer := &KeywordBuffer{}
	buffer.SetKeywords(keywords)
	return buffer
}

func (b *KeywordBuffer) SetKeywords(keywords []string) {
	clean := make([]string, 0, len(keywords))
	for _, keyword := range keywords {
		keyword = strings.TrimSpace(keyword)
		if keyword == "" || len([]rune(keyword)) > KeywordMaximumLength {
			continue
		}
		clean = append(clean, keyword)
	}
	b.keywords = clean
	b.Reset()
}

// Push records one typed character and returns a keyword when a suffix
// completes. The buffer resets after a hit, so the same suffix cannot fire
// twice from one word.
func (b *KeywordBuffer) Push(ch rune, nowMS int64) (string, bool) {
	if b.hasLast && nowMS-b.lastMS > KeywordTimeoutMS {
		b.Reset()
	}
	b.lastMS, b.hasLast = nowMS, true
	b.buffer = append(b.buffer, ch)
	if len(b.buffer) > KeywordMaximumLength {
		b.buffer = b.buffer[len(b.buffer)-KeywordMaximumLength:]
	}
	hit, found := MatchSuffix(string(b.buffer), b.keywords)
	if found {
		b.Reset()
	}
	return hit, found
}

func (b *KeywordBuffer) DeleteBackward(nowMS int64) {
	b.lastMS, b.hasLast = nowMS, true
	if len(b.buffer) > 0 {
		b.buffer = b.buffer[:len(b.buffer)-1]
	}
}

func (b *KeywordBuffer) Reset() {
	b.buffer = b.buffer[:0]
	b.hasLast = false
}

func (b *KeywordBuffer) Empty() bool { return len(b.buffer) == 0 }

// KeywordInput classifies one low-level keyboard transition.
type KeywordInput uint8

const (
	KeywordIgnored KeywordInput = iota
	KeywordText
	KeywordDeleteBackward
	KeywordReset
)

func ClassifyInput(synthetic, keyDown, commanding, resetKey, deleteBackward, producesText bool) KeywordInput {
	switch {
	case synthetic:
		return KeywordIgnored
	case commanding || resetKey:
		return KeywordReset
	case !keyDown:
		return KeywordIgnored
	case deleteBackward:
		return KeywordDeleteBackward
	case producesText:
		return KeywordText
	default:
		return KeywordReset
	}
}
