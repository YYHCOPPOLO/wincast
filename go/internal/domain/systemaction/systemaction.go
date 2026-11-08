// Package systemaction holds the Windows system-action catalog and its
// confirmation policy. It performs no platform calls.
package systemaction

type ID string

const (
	LockScreen           ID = "lock-screen"
	Sleep                ID = "sleep"
	SleepDisplays        ID = "sleep-displays"
	Restart              ID = "restart"
	ShutDown             ID = "shut-down"
	LogOut               ID = "log-out"
	ShowScreenSaver      ID = "show-screen-saver"
	PlayPause            ID = "play-pause"
	NextTrack            ID = "next-track"
	PreviousTrack        ID = "previous-track"
	ToggleMute           ID = "toggle-mute"
	VolumeUp             ID = "volume-up"
	VolumeDown           ID = "volume-down"
	SetVolume            ID = "set-volume"
	Volume0              ID = "volume-0"
	Volume25             ID = "volume-25"
	Volume50             ID = "volume-50"
	Volume75             ID = "volume-75"
	Volume100            ID = "volume-100"
	ShowDesktop          ID = "show-desktop"
	ToggleAppearance     ID = "toggle-system-appearance"
	ToggleStageManager   ID = "toggle-stage-manager"
	OpenTrash            ID = "open-trash"
	EmptyTrash           ID = "empty-trash"
	EjectAllDisks        ID = "eject-all-disks"
	ToggleHiddenFiles    ID = "toggle-hidden-files"
	HideOtherApps        ID = "hide-all-apps-except-frontmost"
	UnhideAllApps        ID = "unhide-all-hidden-apps"
	QuitAllApps          ID = "quit-all-apps"
	DismissNotifications ID = "dismiss-notifications"
	ToggleBluetooth      ID = "toggle-bluetooth"
)

var All = []ID{
	LockScreen, Sleep, SleepDisplays, Restart, ShutDown, LogOut, ShowScreenSaver,
	PlayPause, NextTrack, PreviousTrack, ToggleMute, VolumeUp, VolumeDown, SetVolume,
	Volume0, Volume25, Volume50, Volume75, Volume100, ShowDesktop, ToggleAppearance,
	ToggleStageManager, OpenTrash, EmptyTrash, EjectAllDisks, ToggleHiddenFiles,
	HideOtherApps, UnhideAllApps, QuitAllApps, DismissNotifications, ToggleBluetooth,
}

func (id ID) EntryID() string { return "system-action:" + string(id) }

func FromEntryID(entryID string) (ID, bool) {
	const prefix = "system-action:"
	if len(entryID) <= len(prefix) || entryID[:len(prefix)] != prefix {
		return "", false
	}
	value := ID(entryID[len(prefix):])
	for _, candidate := range All {
		if candidate == value {
			return value, true
		}
	}
	return "", false
}

func (id ID) Name(chinese bool) string {
	if chinese {
		switch id {
		case LockScreen:
			return "锁定屏幕"
		case Sleep:
			return "睡眠"
		case SleepDisplays:
			return "关闭显示器"
		case Restart:
			return "重启"
		case ShutDown:
			return "关机"
		case LogOut:
			return "注销"
		case ShowScreenSaver:
			return "显示屏幕保护"
		case PlayPause:
			return "播放 / 暂停"
		case NextTrack:
			return "下一首"
		case PreviousTrack:
			return "上一首"
		case ToggleMute:
			return "静音"
		case VolumeUp:
			return "提高音量"
		case VolumeDown:
			return "降低音量"
		case SetVolume:
			return "设置音量…"
		case Volume0:
			return "音量设为 0%"
		case Volume25:
			return "音量设为 25%"
		case Volume50:
			return "音量设为 50%"
		case Volume75:
			return "音量设为 75%"
		case Volume100:
			return "音量设为 100%"
		case ShowDesktop:
			return "显示桌面"
		case ToggleAppearance:
			return "切换系统外观"
		case ToggleStageManager:
			return "切换台前调度"
		case OpenTrash:
			return "打开回收站"
		case EmptyTrash:
			return "清空回收站"
		case EjectAllDisks:
			return "弹出所有磁盘"
		case ToggleHiddenFiles:
			return "切换隐藏文件"
		case HideOtherApps:
			return "隐藏其他应用"
		case UnhideAllApps:
			return "取消隐藏所有应用"
		case QuitAllApps:
			return "退出所有应用"
		case DismissNotifications:
			return "清除通知"
		case ToggleBluetooth:
			return "切换蓝牙"
		}
		return string(id)
	}
	switch id {
	case LockScreen:
		return "Lock Screen"
	case Sleep:
		return "Sleep"
	case SleepDisplays:
		return "Sleep Displays"
	case Restart:
		return "Restart"
	case ShutDown:
		return "Shut Down"
	case LogOut:
		return "Log Out"
	case ShowScreenSaver:
		return "Show Screen Saver"
	case PlayPause:
		return "Play / Pause"
	case NextTrack:
		return "Next Track"
	case PreviousTrack:
		return "Previous Track"
	case ToggleMute:
		return "Toggle Mute"
	case VolumeUp:
		return "Turn Volume Up"
	case VolumeDown:
		return "Turn Volume Down"
	case SetVolume:
		return "Set Volume…"
	case Volume0:
		return "Set Volume to 0%"
	case Volume25:
		return "Set Volume to 25%"
	case Volume50:
		return "Set Volume to 50%"
	case Volume75:
		return "Set Volume to 75%"
	case Volume100:
		return "Set Volume to 100%"
	case ShowDesktop:
		return "Show Desktop"
	case ToggleAppearance:
		return "Toggle System Appearance"
	case ToggleStageManager:
		return "Toggle Stage Manager"
	case OpenTrash:
		return "Open Trash"
	case EmptyTrash:
		return "Empty Trash"
	case EjectAllDisks:
		return "Eject All Disks"
	case ToggleHiddenFiles:
		return "Toggle Hidden Files"
	case HideOtherApps:
		return "Hide All Apps Except Frontmost"
	case UnhideAllApps:
		return "Unhide All Hidden Apps"
	case QuitAllApps:
		return "Quit All Applications"
	case DismissNotifications:
		return "Dismiss Notifications"
	case ToggleBluetooth:
		return "Toggle Bluetooth"
	}
	return string(id)
}

// Confirmation describes whether an action needs an explicit confirmation.
type Confirmation struct {
	Required bool
	Computed bool
	Title    string
	Message  string
}

const sessionEndingMessage = "Applications with unsaved changes may ask you to save."

func (id ID) Confirm(chinese bool) Confirmation {
	switch id {
	case Restart:
		if chinese {
			return Confirmation{Required: true, Title: "重启电脑？", Message: "未保存的更改可能提示你保存。"}
		}
		return Confirmation{Required: true, Title: "Restart your PC?", Message: sessionEndingMessage}
	case ShutDown:
		if chinese {
			return Confirmation{Required: true, Title: "关闭电脑？", Message: "未保存的更改可能提示你保存。"}
		}
		return Confirmation{Required: true, Title: "Shut down your PC?", Message: sessionEndingMessage}
	case LogOut:
		if chinese {
			return Confirmation{Required: true, Title: "立即注销？", Message: "未保存的更改可能提示你保存。"}
		}
		return Confirmation{Required: true, Title: "Log out now?", Message: sessionEndingMessage}
	case EmptyTrash:
		if chinese {
			return Confirmation{Required: true, Title: "清空回收站？", Message: "回收站中的项目将被永久删除。"}
		}
		return Confirmation{Required: true, Title: "Empty Trash?", Message: "The items in the Trash will be permanently deleted."}
	case QuitAllApps:
		return Confirmation{Required: true, Computed: true}
	}
	return Confirmation{}
}

func (id ID) ShowsVolume() bool {
	switch id {
	case SetVolume, VolumeUp, VolumeDown, ToggleMute, Volume0, Volume25, Volume50, Volume75, Volume100:
		return true
	}
	return false
}

// Level is the requested scalar volume for the preset actions.
func (id ID) Level() (float64, bool) {
	switch id {
	case Volume0:
		return 0, true
	case Volume25:
		return 0.25, true
	case Volume50:
		return 0.50, true
	case Volume75:
		return 0.75, true
	case Volume100:
		return 1, true
	}
	return 0, false
}
