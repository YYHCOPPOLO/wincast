package clipboard

import (
	"context"
	"strings"
	"sync"

	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

const (
	MaxTextBytes         = 32_000
	DefaultRetentionDays = 90
)

// DefaultDisabledApps keeps password managers out of history by default. A
// user can edit the list in Settings; matching is a case-insensitive executable
// stem comparison.
var DefaultDisabledApps = []string{
	"KeePass",
	"1Password",
	"Bitwarden",
	"LastPass",
	"Dashlane",
}

// Manager connects the Windows clipboard to the SQLite store. Capture is
// called from one dedicated worker goroutine, never the UI loop; the store
// itself is internally synchronized for palette searches.
type Manager struct {
	store        *Store
	mu           sync.RWMutex
	disabled     []string
	lastSequence uint32
}

func NewManager(ctx context.Context, dir, legacy string, disabled []string) (*Manager, error) {
	store, err := Open(ctx, dir, legacy)
	if err != nil {
		return nil, err
	}
	return &Manager{store: store, disabled: append([]string(nil), disabled...)}, nil
}

func (m *Manager) Store() *Store { return m.store }

func (m *Manager) SetDisabledApps(apps []string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.disabled = append(m.disabled[:0], apps...)
}

// Capture reads at most one text or image payload and records it. The private
// marker written by this application is ignored so copy actions do not create
// feedback loops. A clipboard lock race is a transient no-op, not a storage
// failure.
func (m *Manager) Capture() bool {
	if m == nil || m.store == nil || !m.store.Available() {
		return false
	}
	sequence := windows.ClipboardSequence()
	if sequence != 0 && sequence == m.lastSequence {
		return false
	}
	snapshot, ok := windows.ReadClipboardSnapshot(MaxTextBytes, m.sourceAllowed)
	if !ok {
		return false
	}
	if snapshot.Ignored {
		m.lastSequence = snapshot.Sequence
		return false
	}
	if snapshot.Text != "" {
		_, inserted := m.store.InsertText(snapshot.Text)
		if inserted {
			m.lastSequence = snapshot.Sequence
		}
		return inserted
	}
	png, ok := windows.EncodeClipboardImage(snapshot.Image)
	if !ok || len(png) == 0 {
		return false
	}
	_, inserted := m.store.SaveImage(png)
	if inserted {
		m.lastSequence = snapshot.Sequence
	}
	return inserted
}

func (m *Manager) sourceAllowed(stem string) bool {
	stem = normalizedExecutableStem(stem)
	if stem == "" {
		return false
	}
	m.mu.RLock()
	defer m.mu.RUnlock()
	for _, disabled := range m.disabled {
		if normalizedExecutableStem(disabled) == stem {
			return false
		}
	}
	return true
}

func normalizedExecutableStem(name string) string {
	return strings.TrimSuffix(strings.ToLower(strings.TrimSpace(name)), ".exe")
}

func (m *Manager) ApplyRetention(days int64) {
	if m == nil || m.store == nil {
		return
	}
	if days > 0 {
		seconds := days * 86_400
		m.store.SetMaxAgeSeconds(seconds)
		m.store.PruneUnpinnedOlderThan(seconds)
		return
	}
	m.store.SetMaxAgeSeconds(-1)
}

func (m *Manager) Clear() bool {
	if m == nil || m.store == nil {
		return false
	}
	return m.store.Clear()
}

func (m *Manager) Close() error {
	if m == nil || m.store == nil {
		return nil
	}
	return m.store.Close()
}
