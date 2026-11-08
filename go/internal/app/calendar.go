package app

import (
	"log"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/meeting"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/egoist/mygo"
)

// openSchedule shows the agenda. A refresh runs in the background so an
// already-loaded snapshot paints immediately.
func (c *controller) openSchedule() {
	if c.closing {
		return
	}
	if !c.featureCalendar {
		c.view.PendingToast = c.text("日历已关闭", "Calendar is disabled")
		return
	}
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
	}
	c.view.SetModeSchedule()
	c.view.SetScheduleEvents(c.calendar.Agenda(time.Now().Unix()), c.calendarError)
	c.showPanel(false)
	c.refreshCalendar()
}

func (c *controller) startCalendar() {
	if !c.featureCalendar {
		return
	}
	c.startCalendarTicker()
	if c.autoJoin {
		c.calendar.ArmNow(time.Now().Unix())
	}
	c.refreshCalendar()
}

func (c *controller) startCalendarTicker() {
	if c.calendarTickerStarted || c.calendar == nil {
		return
	}
	c.calendarTickerStarted = true
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		ticker := time.NewTicker(time.Minute)
		defer ticker.Stop()
		for {
			select {
			case <-c.ctx.Done():
				return
			case <-ticker.C:
				c.panel.Update(func() {
					if c.closing || !c.featureCalendar {
						return
					}
					c.refreshCalendar()
				})
			}
		}
	}()
}

// refreshCalendar reloads the snapshot and then runs auto join on the UI
// thread. A second request replaces the first; the stale result is dropped by
// the revision check.
func (c *controller) refreshCalendar() {
	if c.calendar == nil || c.closing || c.calendarFetching || !c.featureCalendar {
		return
	}
	c.calendarFetching = true
	c.calendarLoading = true
	if c.view.Mode == palette.ModeSchedule {
		c.view.ScheduleLoading = true
	}
	revision := c.calendarRevision + 1
	c.calendarRevision = revision
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		_, err := c.calendar.Refresh(c.ctx)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing || revision != c.calendarRevision {
				return
			}
			c.calendarFetching = false
			c.calendarLoading = false
			if err != nil {
				c.calendarError = c.calendarErrorText(err)
				log.Printf("calendar refresh failed: %v", err)
				if c.view.Mode == palette.ModeSchedule {
					c.view.SetScheduleEvents(nil, c.calendarError)
				}
				c.refreshSettingsWindow()
				return
			}
			c.calendarError = ""
			if c.view.Mode == palette.ModeSchedule {
				c.view.SetScheduleEvents(c.calendar.Agenda(time.Now().Unix()), "")
			}
			c.refreshSettingsWindow()
			c.runAutoJoin()
		})
	}()
}

// setCalendarEnabled treats the switch as consent: Windows prompts first and
// the setting is written only after the store was returned.
func (c *controller) setCalendarEnabled(enabled bool) {
	if c.closing || c.calendar == nil {
		return
	}
	if !enabled {
		c.featureCalendar = false
		c.calendarRevision++
		c.calendarFetching = false
		c.calendarLoading = false
		c.calendarError = ""
		c.calendar.Clear()
		c.view.ScheduleLoading = false
		if c.view.Mode == palette.ModeSchedule {
			c.view.SetModeLauncher()
		}
		c.persistBool("calendarEnabled", false)
		c.refreshExtras()
		c.refreshSettingsWindow()
		return
	}
	if c.featureCalendar {
		return
	}
	c.calendarLoading = true
	revision := c.calendarRevision + 1
	c.calendarRevision = revision
	c.view.ScheduleLoading = true
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		events, err := c.calendar.RequestAndRefresh(c.ctx)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing || revision != c.calendarRevision {
				return
			}
			c.calendarLoading = false
			if err != nil {
				c.calendarError = c.calendarErrorText(err)
				c.view.ScheduleLoading = false
				log.Printf("calendar permission denied: %v", err)
				c.view.PendingToast = c.calendarError
				c.refreshSettingsWindow()
				return
			}
			c.featureCalendar = true
			c.calendarError = ""
			c.view.ScheduleLoading = false
			c.persistBool("calendarEnabled", true)
			c.startCalendarTicker()
			c.refreshExtras()
			if c.autoJoin {
				c.calendar.ArmNow(time.Now().Unix())
			}
			if c.view.Mode == palette.ModeSchedule {
				c.view.SetScheduleEvents(c.calendar.Agenda(time.Now().Unix()), "")
			} else {
				log.Printf("calendar ready events=%d", len(events))
			}
			c.refreshSettingsWindow()
		})
	}()
}

func (c *controller) refreshSettingsWindow() {
	if c.settings != nil {
		c.settings.Update(func() {})
	}
}

func (c *controller) calendarErrorText(err error) string {
	if windows.CalendarUnavailable(err) {
		return c.text("无法访问日历；请在 Windows 设置中允许日历访问", "Calendar access is unavailable; allow it in Windows Settings")
	}
	text := err.Error()
	if len(text) > 200 {
		text = text[:200]
	}
	return c.text("读取日历失败：", "Could not read the calendar: ") + text
}

// runAutoJoin opens a due meeting at most once. The event is marked joined
// before the confirmation so declining cannot ask again on the next tick.
func (c *controller) runAutoJoin() {
	if c.closing || !c.featureCalendar || !c.autoJoin || c.autoJoinBusy || c.calendar == nil {
		return
	}
	now := time.Now().Unix()
	event, ok := c.calendar.AutoJoin(now, meeting.AutoJoinPolicy{ArmedAt: c.calendar.ArmedAt()})
	if !ok {
		return
	}
	c.calendar.MarkJoined(event.ID)
	if !c.autoJoinConfirms {
		c.joinMeeting(event)
		return
	}
	c.autoJoinBusy = true
	wasVisible := c.panel.IsVisible()
	c.hide()
	options := mygo.MessageOptions{
		Parent:        c.panel,
		Type:          mygo.MessageQuestion,
		Title:         event.Title,
		Message:       c.text("会议现在开始，是否加入？", "Your meeting is starting. Join now?"),
		Detail:        meetingTarget(event),
		Buttons:       []string{c.text("取消", "Cancel"), c.text("加入", "Join")},
		DefaultButton: 1, CancelButton: 0,
	}
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		result, err := mygo.Dialog.Message(options)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			c.autoJoinBusy = false
			if c.closing {
				return
			}
			if err != nil {
				log.Printf("auto join confirmation failed: %v", err)
				if wasVisible {
					c.showPanel(false)
				}
				return
			}
			if result.Button == 1 {
				c.joinMeeting(event)
				return
			}
			if wasVisible {
				c.showPanel(false)
			}
		})
	}()
}

func meetingTarget(event meeting.Event) string {
	if event.Link != nil && event.Link.URL != "" {
		return event.Link.URL
	}
	return "outlookcal:"
}

func (c *controller) joinMeeting(event meeting.Event) {
	if c.closing {
		return
	}
	wasVisible := c.panel.IsVisible()
	c.hide()
	target := meetingTarget(event)
	if err := mygo.Shell.OpenExternal(target); err != nil {
		log.Printf("meeting open failed id=%q error=%v", event.ID, err)
		c.view.Status = c.text("无法打开会议链接", "Could not open the meeting link")
		if wasVisible {
			c.showPanel(false)
		}
		c.showFailure(c.text("无法打开会议链接", "Could not open the meeting link"), event.Title, err.Error())
		return
	}
	provider := "calendar"
	if event.Link != nil {
		provider = string(event.Link.Provider)
	}
	log.Printf("meeting opened id=%q provider=%s", event.ID, provider)
	c.view.Status = c.text("已打开会议", "Meeting opened")
}

func (c *controller) copyMeetingLink(event meeting.Event) {
	if event.Link == nil || event.Link.URL == "" {
		c.view.Status = c.text("该日程没有会议链接", "This event has no meeting link")
		return
	}
	c.copyText(event.Link.URL, false)
}

func (c *controller) selectedSchedule() (meeting.Event, bool) {
	if c.view == nil || c.view.Mode != palette.ModeSchedule {
		return meeting.Event{}, false
	}
	if c.view.Chosen < 0 || c.view.Chosen >= len(c.view.ScheduleItems) {
		return meeting.Event{}, false
	}
	return c.view.ScheduleItems[c.view.Chosen], true
}

func (c *controller) scheduleMenu() {
	selected, ok := c.selectedSchedule()
	hasLink := ok && selected.Link != nil && selected.Link.URL != ""
	menu := components.Menu([]components.MenuAction{
		{Label: c.text("加入会议", "Join meeting"), Disabled: !ok, Click: func() {
			if ok {
				c.joinMeeting(selected)
			}
		}},
		{Label: c.text("复制会议链接", "Copy meeting link"), Accelerator: "Ctrl+Shift+C", Disabled: !hasLink, Click: func() {
			if ok {
				c.copyMeetingLink(selected)
			}
		}},
		{Label: c.text("在日历中打开", "Open in Calendar"), Disabled: !ok, Click: func() {
			if ok {
				c.joinMeeting(meeting.Event{ID: selected.ID, Title: selected.Title})
			}
		}},
		{Separator: true},
		{Label: c.text("刷新日程", "Refresh schedule"), Click: c.refreshCalendar},
		{Label: c.text("返回启动器", "Back to launcher"), Accelerator: "Esc", Click: c.view.SetModeLauncher},
		{Separator: true},
		{Label: c.text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.text("退出预览", "Quit Preview"), Click: mygo.App.Quit},
	})
	menu.Popup(c.panel)
}

func (c *controller) cycleJoinWindow() {
	switch c.joinWindowMinutes {
	case 1:
		c.joinWindowMinutes = 2
	case 2:
		c.joinWindowMinutes = 5
	case 5:
		c.joinWindowMinutes = 10
	case 10:
		c.joinWindowMinutes = 15
	default:
		c.joinWindowMinutes = 1
	}
	if c.calendar != nil {
		c.calendar.SetLeadMinutes(c.joinWindowMinutes)
	}
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("joinWindowMinutes", c.joinWindowMinutes)
	}
	c.queueSettingsSave()
	c.refreshSettingsWindow()
}

func (c *controller) persistBool(key string, value bool) {
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set(key, value)
	}
	c.queueSettingsSave()
}
