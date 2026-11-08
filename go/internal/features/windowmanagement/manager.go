// Package windowmanagement owns the per-window restore/cycle memory and
// forwards placement requests to the Windows mover.
package windowmanagement

import (
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowcommand"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowmemory"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

type Manager struct {
	memory *windowmemory.Memory
}

func New() *Manager {
	return &Manager{memory: windowmemory.New()}
}

func (m *Manager) Perform(hwnd uintptr, id windowcommand.ID, gap float64, cycle bool) (bool, string) {
	return windows.ApplyWindowCommand(hwnd, id, gap, cycle, m.memory)
}

func WindowTarget(previous uintptr, panelVisible, ownsForeground bool) uintptr {
	if panelVisible || ownsForeground {
		return previous
	}
	foreground := windows.ForegroundWindow()
	if foreground == 0 {
		return previous
	}
	return foreground
}
