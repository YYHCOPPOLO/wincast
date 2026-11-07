package snippets

import (
	"sync"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/snippet"
)

// Input is one classified keyboard transition. The platform supplies the
// character and flags; the expander owns only the bounded buffer.
type Input struct {
	Synthetic      bool
	KeyDown        bool
	Commanding     bool
	ResetKey       bool
	DeleteBackward bool
	ProducesText   bool
	Composing      bool
	Character      rune
	Foreground     uintptr
	NowMS          int64
}

// Expander matches configured keywords against recently typed characters.
// It never stores a key stream: only the current bounded suffix is retained
// and it is cleared on every gate.
type Expander struct {
	mu       sync.Mutex
	buffer   *snippet.KeywordBuffer
	keywords []string
	last     uintptr
	enabled  bool
}

func NewExpander() *Expander {
	return &Expander{buffer: snippet.NewKeywordBuffer(nil)}
}

// SetKeywords replaces the match set and resets any partial input.
func (e *Expander) SetKeywords(keywords []string) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.keywords = append([]string(nil), keywords...)
	e.buffer.SetKeywords(e.keywords)
	e.last = 0
}

func (e *Expander) SetEnabled(enabled bool) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.enabled = enabled
	if !enabled {
		e.buffer.Reset()
		e.last = 0
	}
}

func (e *Expander) Enabled() bool {
	e.mu.Lock()
	defer e.mu.Unlock()
	return e.enabled
}

func (e *Expander) Reset() {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.buffer.Reset()
	e.last = 0
}

// Feed classifies one transition and reports a completed keyword. The buffer
// is cleared on composition, modifier chords, navigation, foreground changes
// and disabling, so a stale suffix can never expand in another app.
func (e *Expander) Feed(input Input) (string, bool) {
	e.mu.Lock()
	defer e.mu.Unlock()
	if !e.enabled {
		return "", false
	}
	if input.Composing {
		e.buffer.Reset()
		return "", false
	}
	if e.last != 0 && input.Foreground != 0 && e.last != input.Foreground {
		e.buffer.Reset()
	}
	if input.Foreground != 0 {
		e.last = input.Foreground
	}
	class := snippet.ClassifyInput(
		input.Synthetic, input.KeyDown, input.Commanding, input.ResetKey,
		input.DeleteBackward, input.ProducesText,
	)
	switch class {
	case snippet.KeywordDeleteBackward:
		e.buffer.DeleteBackward(input.NowMS)
	case snippet.KeywordText:
		return e.buffer.Push(input.Character, input.NowMS)
	case snippet.KeywordReset:
		e.buffer.Reset()
	}
	return "", false
}
