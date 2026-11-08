// Package quickactions runs one selection transformer at a time and owns the
// result state that the small preview window reads.
package quickactions

import (
	"context"
	"strings"
	"sync"

	domainai "github.com/YYHCOPPOLO/wincast/go/internal/domain/ai"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

// Snapshot is a consistent copy for the UI thread.
type Snapshot struct {
	Generation uint64
	Action     domainai.QuickAction
	Target     windows.Target
	Result     string
	Running    bool
	Accepted   bool
	Canceled   bool
	Failed     bool
	Preview    bool
}

type Coordinator struct {
	mu         sync.Mutex
	generation uint64
	action     domainai.QuickAction
	target     windows.Target
	result     strings.Builder
	running    bool
	accepted   bool
	canceled   bool
	failed     bool
	preview    bool
	cancel     context.CancelFunc
}

func New() *Coordinator { return &Coordinator{} }

// Begin starts one action and cancels any previous stream. The returned
// generation keeps late events from a replaced stream out of the new result.
func (c *Coordinator) Begin(action domainai.QuickAction, target windows.Target, preview bool) (context.Context, uint64) {
	ctx, cancel := context.WithCancel(context.Background())
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.cancel != nil {
		c.cancel()
	}
	c.generation++
	c.action = action
	c.target = target
	c.result.Reset()
	c.running = true
	c.accepted = false
	c.canceled = false
	c.failed = false
	c.preview = preview
	c.cancel = cancel
	return ctx, c.generation
}

func (c *Coordinator) Event(generation uint64, event domainai.Event) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if generation != c.generation || c.canceled {
		return
	}
	switch event.Kind {
	case domainai.EventText:
		c.result.WriteString(event.Text)
	case domainai.EventDone:
		c.running = false
		c.accepted = true
	case domainai.EventError:
		c.running = false
		c.accepted = false
		c.failed = true
		if c.result.Len() == 0 {
			c.result.WriteString(event.Text)
		}
	}
}

func (c *Coordinator) Cancel() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.canceled = true
	c.running = false
	c.accepted = false
	if c.cancel != nil {
		c.cancel()
		c.cancel = nil
	}
}

func (c *Coordinator) Snapshot() Snapshot {
	c.mu.Lock()
	defer c.mu.Unlock()
	return Snapshot{
		Generation: c.generation,
		Action:     c.action, Target: c.target, Result: c.result.String(),
		Running: c.running, Accepted: c.accepted, Canceled: c.canceled,
		Failed: c.failed, Preview: c.preview,
	}
}

// Apply replaces the remembered selection only when the stream completed and
// the target identity is still the one captured before the palette hid.
func (c *Coordinator) Apply(ctx context.Context, owner uintptr) error {
	c.mu.Lock()
	snapshot := Snapshot{
		Target: c.target, Result: c.result.String(),
		Accepted: c.accepted, Canceled: c.canceled,
	}
	c.mu.Unlock()
	if snapshot.Canceled || !snapshot.Accepted || strings.TrimSpace(snapshot.Result) == "" {
		return ErrNotReady
	}
	return windows.ReplaceSelection(ctx, snapshot.Target, owner, snapshot.Result)
}

func (c *Coordinator) Reset() {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.cancel != nil {
		c.cancel()
		c.cancel = nil
	}
	c.generation++
	c.result.Reset()
	c.running = false
	c.accepted = false
	c.canceled = false
	c.preview = false
}

var ErrNotReady = errQuickActionNotReady{}

type errQuickActionNotReady struct{}

func (errQuickActionNotReady) Error() string { return "the quick action result is not ready" }
