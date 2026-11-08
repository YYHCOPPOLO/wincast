// Package calendar owns the appointment snapshot, the permission state and
// the launch-scoped join memory. The platform fetch runs off the UI thread;
// every read here is a cheap copy so the view can render from it.
package calendar

import (
	"context"
	"sync"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/meeting"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

type Permission uint8

const (
	PermissionUnknown Permission = iota
	PermissionGranted
	PermissionDenied
)

type Store struct {
	mu         sync.Mutex
	window     meeting.UpcomingWindow
	events     []meeting.Event
	joined     map[string]bool
	armedAt    int64
	permission Permission
	err        error
}

func New(leadMinutes int64) *Store {
	return &Store{
		window:     meeting.UpcomingWindow{LeadMinutes: leadMinutes},
		joined:     make(map[string]bool),
		permission: PermissionUnknown,
	}
}

func (s *Store) SetLeadMinutes(minutes int64) {
	if minutes <= 0 {
		minutes = 5
	}
	s.mu.Lock()
	s.window.LeadMinutes = minutes
	s.mu.Unlock()
}

func (s *Store) LeadMinutes() int64 {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.window.LeadMinutes
}

// RequestAndRefresh asks Windows for the read-only appointment store. The
// first call can raise the system consent prompt, so it must not run on the
// UI thread. An error is a denied or unavailable permission, never an empty
// calendar.
func (s *Store) RequestAndRefresh(ctx context.Context) ([]meeting.Event, error) {
	return s.load(ctx)
}

// Refresh updates the snapshot without asking again for consent. A permission
// error still flips the state to denied so the screen cannot show stale rows
// as current.
func (s *Store) Refresh(ctx context.Context) ([]meeting.Event, error) {
	return s.load(ctx)
}

func (s *Store) load(ctx context.Context) ([]meeting.Event, error) {
	events, err := windows.FetchAppointments(ctx)
	s.mu.Lock()
	defer s.mu.Unlock()
	if err != nil {
		s.events = nil
		s.err = err
		if windows.CalendarUnavailable(err) {
			s.permission = PermissionDenied
		}
		return nil, err
	}
	s.events = events
	s.err = nil
	s.permission = PermissionGranted
	return clone(events), nil
}

// Clear drops the rows and the permission state, used when the feature is
// switched off.
func (s *Store) Clear() {
	s.mu.Lock()
	s.events = nil
	s.err = nil
	s.permission = PermissionUnknown
	s.joined = make(map[string]bool)
	s.mu.Unlock()
}

func (s *Store) Agenda(now int64) []meeting.Event {
	s.mu.Lock()
	window, events := s.window, clone(s.events)
	s.mu.Unlock()
	return window.Agenda(events, now)
}

func (s *Store) Carded(now int64) (meeting.Event, bool) {
	s.mu.Lock()
	window, events := s.window, clone(s.events)
	s.mu.Unlock()
	return window.Carded(events, now)
}

func (s *Store) Joinable(now int64) (meeting.Event, bool) {
	s.mu.Lock()
	window, events := s.window, clone(s.events)
	s.mu.Unlock()
	return window.Joinable(events, now)
}

func (s *Store) AutoJoin(now int64, policy meeting.AutoJoinPolicy) (meeting.Event, bool) {
	s.mu.Lock()
	window, events := s.window, clone(s.events)
	joined := make(map[string]bool, len(s.joined))
	for id, value := range s.joined {
		joined[id] = value
	}
	s.mu.Unlock()
	return policy.Meeting(events, now, window, joined)
}

func (s *Store) MarkJoined(id string) {
	if id == "" {
		return
	}
	s.mu.Lock()
	s.joined[id] = true
	s.mu.Unlock()
}

func (s *Store) Joined(id string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.joined[id]
}

func (s *Store) ArmNow(now int64) {
	s.mu.Lock()
	s.armedAt = now
	s.mu.Unlock()
}

func (s *Store) ArmedAt() int64 {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.armedAt
}

func (s *Store) Permission() Permission {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.permission
}

func (s *Store) Err() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.err
}

func (s *Store) Events() []meeting.Event {
	s.mu.Lock()
	defer s.mu.Unlock()
	return clone(s.events)
}

func clone(events []meeting.Event) []meeting.Event {
	if len(events) == 0 {
		return nil
	}
	out := make([]meeting.Event, len(events))
	copy(out, events)
	return out
}
