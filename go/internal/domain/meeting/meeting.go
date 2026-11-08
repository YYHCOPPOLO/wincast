// Package meeting holds calendar events, meeting-link detection and the
// upcoming-window policy. Clock values are always injected.
package meeting

import (
	"fmt"
	"strings"
)

type Provider string

const (
	ProviderZoom        Provider = "zoom"
	ProviderGoogleMeet  Provider = "google-meet"
	ProviderTeams       Provider = "teams"
	ProviderWebex       Provider = "webex"
	ProviderJitsi       Provider = "jitsi"
	ProviderWhereby     Provider = "whereby"
	ProviderChime       Provider = "chime"
	ProviderGotoMeeting Provider = "goto-meeting"
	ProviderBlueJeans   Provider = "blue-jeans"
	ProviderSkype       Provider = "skype"
	ProviderGeneric     Provider = "generic"
)

type Link struct {
	Provider Provider
	URL      string
}

type Event struct {
	ID             string
	Title          string
	Start          int64
	End            int64
	AllDay         bool
	Declined       bool
	CalendarID     string
	CalendarName   string
	CalendarItemID string
	Link           *Link
}

func (e Event) InProgress(now int64) bool {
	return e.Start <= now && now < e.End
}

type UpcomingWindow struct {
	LeadMinutes int64
}

func (w UpcomingWindow) leadSeconds() int64 { return w.LeadMinutes * 60 }

func (w UpcomingWindow) Agenda(events []Event, now int64) []Event {
	out := make([]Event, 0, len(events))
	for _, event := range events {
		if !event.AllDay && !event.Declined && event.End > now {
			out = append(out, event)
		}
	}
	// Stable insertion sort by start keeps the oracle's deterministic order.
	for i := 1; i < len(out); i++ {
		for j := i; j > 0 && out[j].Start < out[j-1].Start; j-- {
			out[j], out[j-1] = out[j-1], out[j]
		}
	}
	return out
}

func (w UpcomingWindow) Carded(events []Event, now int64) (Event, bool) {
	lead := w.leadSeconds()
	for _, event := range w.Agenda(events, now) {
		if event.Link == nil {
			continue
		}
		end := min(event.Start+lead, event.End)
		if now >= event.Start-lead && now < end {
			return event, true
		}
	}
	return Event{}, false
}

func (w UpcomingWindow) Joinable(events []Event, now int64) (Event, bool) {
	if event, ok := w.Carded(events, now); ok {
		return event, true
	}
	linked := make([]Event, 0, len(events))
	for _, event := range w.Agenda(events, now) {
		if event.Link != nil {
			linked = append(linked, event)
		}
	}
	for _, event := range linked {
		if event.InProgress(now) {
			return event, true
		}
	}
	for _, event := range linked {
		if event.Start > now {
			return event, true
		}
	}
	return Event{}, false
}

func (w UpcomingWindow) Countdown(start, now int64) string {
	delta := start - now
	if delta > 0 {
		return fmt.Sprintf("in %d min", (delta+59)/60)
	}
	elapsed := -delta / 60
	if elapsed == 0 {
		return "now"
	}
	return fmt.Sprintf("%d min ago", elapsed)
}

type AutoJoinPolicy struct {
	ArmedAt int64
}

func (p AutoJoinPolicy) Meeting(events []Event, now int64, window UpcomingWindow, joined map[string]bool) (Event, bool) {
	event, ok := window.Carded(events, now)
	if !ok {
		return Event{}, false
	}
	if event.Start >= p.ArmedAt && now >= event.Start && !joined[event.ID] {
		return event, true
	}
	return Event{}, false
}

// DetectLink finds the best meeting link across fields in precedence order. A
// named provider anywhere beats a bare link found earlier.
func DetectLink(fields []string) *Link {
	var generic *Link
	for _, field := range fields {
		for _, url := range webURLs(field) {
			link := classify(url)
			if link == nil {
				continue
			}
			if link.Provider != ProviderGeneric {
				return link
			}
			if generic == nil {
				generic = link
			}
		}
	}
	return generic
}

func webURLs(text string) []string {
	var urls []string
	lower := strings.ToLower(text)
	for offset := 0; offset < len(lower); {
		index := strings.Index(lower[offset:], "http")
		if index < 0 {
			break
		}
		start := offset + index
		end := start
		for end < len(text) {
			r := text[end]
			if r == ' ' || r == '\t' || r == '\n' || r == '\r' || r == '"' || r == '\'' ||
				r == '<' || r == '>' || r == '«' || r == '»' || r == 0xC2 {
				break
			}
			end++
		}
		value := strings.TrimRight(text[start:end], ".,;:)]}!")
		for _, prefix := range []string{"https://", "http://"} {
			if strings.HasPrefix(strings.ToLower(value), prefix) {
				urls = append(urls, value)
				break
			}
		}
		offset = end
	}
	return urls
}

func classify(url string) *Link {
	lower := strings.ToLower(url)
	host := ""
	if rest, ok := strings.CutPrefix(lower, "https://"); ok {
		host = rest
	} else if rest, ok := strings.CutPrefix(lower, "http://"); ok {
		host = rest
	}
	if slash := strings.IndexAny(host, "/?#"); slash >= 0 {
		host = host[:slash]
	}
	if host == "" {
		return nil
	}
	provider := ProviderGeneric
	switch {
	case host == "zoom.us" || strings.HasSuffix(host, ".zoom.us") || host == "zoom.com" || strings.HasSuffix(host, ".zoom.com"):
		provider = ProviderZoom
	case host == "meet.google.com":
		provider = ProviderGoogleMeet
	case host == "teams.microsoft.com" || host == "teams.live.com":
		provider = ProviderTeams
	case host == "webex.com" || strings.HasSuffix(host, ".webex.com"):
		provider = ProviderWebex
	case host == "meet.jit.si":
		provider = ProviderJitsi
	case host == "whereby.com" || strings.HasSuffix(host, ".whereby.com"):
		provider = ProviderWhereby
	case host == "chime.aws":
		provider = ProviderChime
	case host == "gotomeeting.com" || host == "goto.com" || strings.HasSuffix(host, ".gotomeeting.com"):
		provider = ProviderGotoMeeting
	case host == "bluejeans.com" || strings.HasSuffix(host, ".bluejeans.com"):
		provider = ProviderBlueJeans
	case host == "skype.com" || host == "join.skype.com":
		provider = ProviderSkype
	}
	return &Link{Provider: provider, URL: url}
}
