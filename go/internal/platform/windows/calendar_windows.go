//go:build windows

package windows

import (
	"context"
	"errors"
	"fmt"
	"time"
	"unsafe"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/meeting"
)

var (
	iidAppointmentManagerStatics2 = guidValue(0x0A81F60D, 0xD04F, 0x4034, [8]byte{0xAF, 0x72, 0xA3, 0x65, 0x73, 0xB4, 0x5F, 0xF0})
	iidAppointment2               = guidValue(0x5E85983C, 0x540F, 0x3452, [8]byte{0x9B, 0x5C, 0x0D, 0xD7, 0xAD, 0x4C, 0x65, 0xA2})
	getCurrentPackageFullName     = kernel32.NewProc("GetCurrentPackageFullName")
)

const (
	appointmentAllCalendarsReadOnly = 1
	appointmentResponseDeclined     = 3
	appointmentMaxCount             = 200
	appointmentRequestTimeout       = 10 * time.Second
	unixToFileTimeSeconds           = 11_644_473_600
	errorInsufficientBuffer         = 122
)

// IAppointment slots after the six IInspectable entries.
const (
	slotAppointmentStartTime    = 6
	slotAppointmentDuration     = 8
	slotAppointmentLocation     = 10
	slotAppointmentSubject      = 12
	slotAppointmentDetails      = 14
	slotAppointmentAllDay       = 25
	slotAppointment2LocalID     = 30
	slotAppointment2CalendarID  = 31
	slotAppointment2OnlineLink  = 39
	slotAppointment2UserRespond = 43
	slotAppointment2Canceled    = 46
)

// Appointment property names requested from FindAppointmentsOptions. The
// default overload leaves Subject/Details empty for several providers.
var appointmentProperties = []string{
	"Subject", "Location", "Details", "StartTime", "Duration", "AllDay",
	"OnlineMeetingLink", "UserResponse", "IsCanceledMeeting", "LocalId", "CalendarId",
}

// FetchAppointments requests the read-only store and returns the next three
// days. The caller must treat an error as a denied/unavailable permission
// state, not as an empty calendar.
func FetchAppointments(ctx context.Context) ([]meeting.Event, error) {
	if !packagedIdentity() {
		// AppointmentManager requires the appointmentsSystem restricted
		// capability, which is only granted to a packaged application. Calling
		// it unpackaged leaves the async request pending forever.
		return nil, fmt.Errorf("%w: a packaged identity is required", errCalendarUnavailable)
	}
	var events []meeting.Event
	err := withWinRT(func() error {
		factory, err := activationFactory("Windows.ApplicationModel.Appointments.AppointmentManager", &iidAppointmentManagerStatics2)
		if err != nil {
			return err
		}
		defer comRelease(factory)
		var request uintptr
		if hr := comCall(factory, 9, appointmentAllCalendarsReadOnly, uintptr(unsafe.Pointer(&request))); int32(hr) < 0 || request == 0 {
			return fmt.Errorf("AppointmentManager.RequestStoreAsync: 0x%08x", uint32(hr))
		}
		store, err := awaitOperation(request, appointmentRequestTimeout)
		comRelease(request)
		if err != nil {
			return err
		}
		defer comRelease(store)
		names := calendarNames(store)
		options, err := appointmentOptions()
		if err != nil {
			return err
		}
		defer comRelease(options)
		start := startOfToday()
		duration := int64(3 * 24 * 60 * 60 * 10_000_000)
		var operation uintptr
		if hr := comCall(store, 14, uintptr(start), uintptr(duration), options, uintptr(unsafe.Pointer(&operation))); int32(hr) < 0 || operation == 0 {
			return fmt.Errorf("AppointmentStore.FindAppointmentsAsync: 0x%08x", uint32(hr))
		}
		view, err := awaitOperation(operation, appointmentRequestTimeout)
		comRelease(operation)
		if err != nil {
			return err
		}
		defer comRelease(view)
		var size uint32
		if hr := comCall(view, 7, uintptr(unsafe.Pointer(&size))); int32(hr) < 0 {
			return fmt.Errorf("IVectorView.Size: 0x%08x", uint32(hr))
		}
		for index := uint32(0); index < size; index++ {
			if err := ctx.Err(); err != nil {
				return err
			}
			var appointment uintptr
			if hr := comCall(view, 6, uintptr(index), uintptr(unsafe.Pointer(&appointment))); int32(hr) < 0 || appointment == 0 {
				continue
			}
			event, ok := appointmentEvent(appointment, names)
			comRelease(appointment)
			if ok {
				events = append(events, event)
			}
		}
		return nil
	})
	if err != nil {
		if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
			return nil, err
		}
		return nil, fmt.Errorf("%w: %v", errCalendarUnavailable, err)
	}
	if events == nil {
		events = []meeting.Event{}
	}
	return events, nil
}

// packagedIdentity reports whether the process has a package identity. The
// probe is silent and never prompts.
func packagedIdentity() bool {
	var length uint32
	hr, _, _ := getCurrentPackageFullName.Call(uintptr(unsafe.Pointer(&length)), 0)
	return uint32(hr) == errorInsufficientBuffer
}

func appointmentOptions() (uintptr, error) {
	options, err := activateInstance("Windows.ApplicationModel.Appointments.FindAppointmentsOptions")
	if err != nil {
		return 0, err
	}
	var properties uintptr
	if hr := comCall(options, 7, uintptr(unsafe.Pointer(&properties))); int32(hr) < 0 || properties == 0 {
		comRelease(options)
		return 0, fmt.Errorf("FindAppointmentsOptions.FetchProperties: 0x%08x", uint32(hr))
	}
	defer comRelease(properties)
	for _, property := range appointmentProperties {
		name, err := newHString(property)
		if err != nil {
			comRelease(options)
			return 0, err
		}
		hr := comCall(properties, 13, uintptr(name))
		name.Close()
		if int32(hr) < 0 {
			comRelease(options)
			return 0, fmt.Errorf("FetchProperties.Append(%s): 0x%08x", property, uint32(hr))
		}
	}
	comCall(options, 11, appointmentMaxCount) // MaxCount, 0 disables the cap.
	return options, nil
}

// calendarNames maps calendar IDs to their display names so a row can name its
// calendar without a second query per event.
func calendarNames(store uintptr) map[string]string {
	names := make(map[string]string, 4)
	var operation uintptr
	if hr := comCall(store, 11, uintptr(unsafe.Pointer(&operation))); int32(hr) < 0 || operation == 0 {
		return names
	}
	view, err := awaitOperation(operation, appointmentRequestTimeout)
	comRelease(operation)
	if err != nil {
		return names
	}
	defer comRelease(view)
	var size uint32
	if hr := comCall(view, 7, uintptr(unsafe.Pointer(&size))); int32(hr) < 0 {
		return names
	}
	for index := uint32(0); index < size; index++ {
		var calendar uintptr
		if hr := comCall(view, 6, uintptr(index), uintptr(unsafe.Pointer(&calendar))); int32(hr) < 0 || calendar == 0 {
			continue
		}
		name := appointmentStringAt(calendar, 7)
		id := appointmentStringAt(calendar, 9)
		comRelease(calendar)
		if id != "" && name != "" {
			names[id] = name
		}
	}
	return names
}

func appointmentEvent(appointment uintptr, names map[string]string) (meeting.Event, bool) {
	var startTime int64
	if hr := comCall(appointment, slotAppointmentStartTime, uintptr(unsafe.Pointer(&startTime))); int32(hr) < 0 {
		return meeting.Event{}, false
	}
	var duration int64
	if hr := comCall(appointment, slotAppointmentDuration, uintptr(unsafe.Pointer(&duration))); int32(hr) < 0 {
		return meeting.Event{}, false
	}
	subject := appointmentStringAt(appointment, slotAppointmentSubject)
	location := appointmentStringAt(appointment, slotAppointmentLocation)
	details := appointmentStringAt(appointment, slotAppointmentDetails)
	var allDay bool
	comCall(appointment, slotAppointmentAllDay, uintptr(unsafe.Pointer(&allDay)))

	localID, calendarID, onlineLink := "", "", ""
	declined, canceled := false, false
	if extended := comQueryInterface(appointment, &iidAppointment2); extended != 0 {
		localID = appointmentStringAt(extended, slotAppointment2LocalID)
		calendarID = appointmentStringAt(extended, slotAppointment2CalendarID)
		onlineLink = appointmentStringAt(extended, slotAppointment2OnlineLink)
		var response int32
		if hr := comCall(extended, slotAppointment2UserRespond, uintptr(unsafe.Pointer(&response))); int32(hr) == 0 {
			declined = response == appointmentResponseDeclined
		}
		var flag bool
		if hr := comCall(extended, slotAppointment2Canceled, uintptr(unsafe.Pointer(&flag))); int32(hr) == 0 {
			canceled = flag
		}
		comRelease(extended)
	}
	if canceled {
		return meeting.Event{}, false
	}
	link := meeting.DetectLink([]string{onlineLink, location, details})
	start := startTime/10_000_000 - unixToFileTimeSeconds
	seconds := duration / 10_000_000
	if start <= 0 {
		return meeting.Event{}, false
	}
	id := fmt.Sprintf("%s:%d", subject, start)
	if localID != "" {
		id = fmt.Sprintf("%s:%d", localID, start)
	}
	return meeting.Event{
		ID: id, Title: subject, Start: start, End: start + seconds,
		AllDay: allDay, Declined: declined,
		CalendarID: calendarID, CalendarName: names[calendarID], CalendarItemID: localID, Link: link,
	}, true
}

func appointmentStringAt(object uintptr, slot int) string {
	var value hstring
	if hr := comCall(object, slot, uintptr(unsafe.Pointer(&value))); int32(hr) < 0 || value == 0 {
		return ""
	}
	defer value.Close()
	return value.String()
}

func startOfToday() int64 {
	now := time.Now()
	midnight := time.Date(now.Year(), now.Month(), now.Day(), 0, 0, 0, 0, now.Location())
	return (midnight.Unix() + unixToFileTimeSeconds) * 10_000_000
}

var errCalendarUnavailable = errors.New("calendar is unavailable")

// CalendarUnavailable exposes a stable error for callers that distinguish
// permission failure from an empty event list.
func CalendarUnavailable(err error) bool {
	return errors.Is(err, errCalendarUnavailable)
}
