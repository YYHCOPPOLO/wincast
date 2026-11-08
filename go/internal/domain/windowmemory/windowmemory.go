// Package windowmemory keeps one restore frame and cycle step per window.
package windowmemory

import (
	"math"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowcommand"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowlayout"
)

const tolerance = 2.0

type Record struct {
	Restore  windowlayout.DipRect
	Applied  windowlayout.DipRect
	Command  windowcommand.ID
	Step     int
	ScreenID int
	AtMS     int64
}

type Decision struct {
	Step       int
	Restore    windowlayout.DipRect
	CanRestore bool
	LastTile   windowcommand.ID
}

type Memory struct {
	Capacity       int
	CycleTimeoutMS int64
	records        map[uintptr]Record
	order          []uintptr
}

func New() *Memory {
	return &Memory{Capacity: 64, records: make(map[uintptr]Record)}
}

func (m *Memory) Count() int { return len(m.records) }

func (m *Memory) Record(key uintptr) (Record, bool) {
	record, ok := m.records[key]
	return record, ok
}

func (m *Memory) Decide(key uintptr, command windowcommand.ID, current windowlayout.DipRect, screenID int, cycleEnabled bool, nowMS int64) Decision {
	record, ok := m.records[key]
	if !ok {
		return Decision{Restore: current}
	}
	if !approx(current, record.Applied) {
		return Decision{Restore: current, CanRestore: true}
	}
	lastTile := windowcommand.ID("")
	if windowlayout.IsTileCommand(record.Command) {
		lastTile = record.Command
	}
	expired := m.CycleTimeoutMS > 0 && nowMS-record.AtMS > m.CycleTimeoutMS
	continues := cycleEnabled && command.CyclesOnRepeat() && command == record.Command &&
		screenID == record.ScreenID && !expired
	step := 0
	if continues {
		step = (record.Step + 1) % 3
	}
	return Decision{Step: step, Restore: record.Restore, CanRestore: true, LastTile: lastTile}
}

func (m *Memory) Commit(key uintptr, command windowcommand.ID, decision Decision, applied windowlayout.DipRect, screenID int, nowMS int64) {
	m.records[key] = Record{
		Restore: decision.Restore, Applied: applied, Command: command,
		Step: decision.Step, ScreenID: screenID, AtMS: nowMS,
	}
	m.touch(key)
}

func (m *Memory) Forget(key uintptr) {
	if _, ok := m.records[key]; ok {
		delete(m.records, key)
		for i, candidate := range m.order {
			if candidate == key {
				m.order = append(m.order[:i], m.order[i+1:]...)
				break
			}
		}
	}
}

func (m *Memory) touch(key uintptr) {
	for i, candidate := range m.order {
		if candidate == key {
			m.order = append(m.order[:i], m.order[i+1:]...)
			break
		}
	}
	m.order = append(m.order, key)
	for len(m.order) > m.Capacity {
		oldest := m.order[0]
		m.order = m.order[1:]
		delete(m.records, oldest)
	}
}

func approx(a, b windowlayout.DipRect) bool {
	return math.Abs(a.X-b.X) <= tolerance && math.Abs(a.Y-b.Y) <= tolerance &&
		math.Abs(a.W-b.W) <= tolerance && math.Abs(a.H-b.H) <= tolerance
}
