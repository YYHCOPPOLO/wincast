// Package settings holds the pure settings-window model: the tab order and
// labels the original application used, plus copy that does not depend on UI
// or disk state.
package settings

// Tab is one page of the settings window. The order matches the original
// application so muscle memory and screenshots stay comparable.
type Tab uint8

const (
	General Tab = iota
	Applications
	SystemSettings
	SystemActions
	Commands
	Quicklinks
	AI
	QuickActions
	FileSearch
	Notes
	Snippets
	WindowManagement
	Clipboard
	Emoji
	Calendar
	Extensions
	Permissions
	Backup
	About
)

// TabCount is the number of settings pages.
const TabCount = int(About) + 1

// Section groups tabs in the sidebar.
type Section uint8

const (
	SectionGeneral Section = iota
	SectionLauncher
	SectionFeatures
	SectionAdvanced
)

// AllTabs lists every page in sidebar order.
func AllTabs() []Tab {
	tabs := make([]Tab, 0, TabCount)
	for tab := General; tab <= About; tab++ {
		tabs = append(tabs, tab)
	}
	return tabs
}

// Tabs returns the pages of one sidebar section.
func (s Section) Tabs() []Tab {
	switch s {
	case SectionGeneral:
		return []Tab{General, Permissions}
	case SectionLauncher:
		return []Tab{Applications, SystemSettings, SystemActions, Commands, Quicklinks}
	case SectionFeatures:
		return []Tab{AI, QuickActions, FileSearch, Notes, Snippets, WindowManagement, Clipboard, Emoji, Calendar, Extensions}
	default:
		return []Tab{Backup, About}
	}
}

// Title is the section header shown above its tabs.
func (s Section) Title(zh bool) string {
	if zh {
		switch s {
		case SectionGeneral:
			return "通用"
		case SectionLauncher:
			return "启动器"
		case SectionFeatures:
			return "功能"
		default:
			return "高级"
		}
	}
	switch s {
	case SectionGeneral:
		return "General"
	case SectionLauncher:
		return "Launcher"
	case SectionFeatures:
		return "Features"
	default:
		return "Advanced"
	}
}

// AllSections lists sidebar sections top to bottom.
func AllSections() []Section {
	return []Section{SectionGeneral, SectionLauncher, SectionFeatures, SectionAdvanced}
}

// ID is the stable identifier used by the sidebar widget and by persisted
// state; it never changes with the visible language.
func (t Tab) ID() string {
	switch t {
	case General:
		return "general"
	case Applications:
		return "applications"
	case SystemSettings:
		return "system-settings"
	case SystemActions:
		return "system-actions"
	case Commands:
		return "commands"
	case Quicklinks:
		return "quicklinks"
	case AI:
		return "ai"
	case QuickActions:
		return "quick-actions"
	case FileSearch:
		return "file-search"
	case Notes:
		return "notes"
	case Snippets:
		return "snippets"
	case WindowManagement:
		return "window-management"
	case Clipboard:
		return "clipboard"
	case Emoji:
		return "emoji"
	case Calendar:
		return "calendar"
	case Extensions:
		return "extensions"
	case Permissions:
		return "permissions"
	case Backup:
		return "backup"
	case About:
		return "about"
	}
	return ""
}

// TabFromID resolves a sidebar selection back to a page.
func TabFromID(id string) (Tab, bool) {
	for _, tab := range AllTabs() {
		if tab.ID() == id {
			return tab, true
		}
	}
	return General, false
}

// Title is the localized page heading.
func (t Tab) Title(zh bool) string {
	if zh {
		switch t {
		case General:
			return "通用"
		case Applications:
			return "应用"
		case SystemSettings:
			return "系统设置"
		case SystemActions:
			return "系统操作"
		case Commands:
			return "命令"
		case Quicklinks:
			return "快捷链接"
		case AI:
			return "AI"
		case QuickActions:
			return "快捷操作"
		case FileSearch:
			return "文件搜索"
		case Notes:
			return "笔记"
		case Snippets:
			return "片段"
		case WindowManagement:
			return "窗口管理"
		case Clipboard:
			return "剪贴板"
		case Emoji:
			return "表情与符号"
		case Calendar:
			return "日历"
		case Extensions:
			return "扩展"
		case Permissions:
			return "权限"
		case Backup:
			return "备份"
		case About:
			return "关于"
		}
		return ""
	}
	switch t {
	case General:
		return "General"
	case Applications:
		return "Applications"
	case SystemSettings:
		return "System Settings"
	case SystemActions:
		return "System Actions"
	case Commands:
		return "Commands"
	case Quicklinks:
		return "Quicklinks"
	case AI:
		return "AI"
	case QuickActions:
		return "Quick Actions"
	case FileSearch:
		return "File Search"
	case Notes:
		return "Notes"
	case Snippets:
		return "Snippets"
	case WindowManagement:
		return "Window Management"
	case Clipboard:
		return "Clipboard"
	case Emoji:
		return "Emoji & Symbols"
	case Calendar:
		return "Calendar"
	case Extensions:
		return "Extensions"
	case Permissions:
		return "Permissions"
	case Backup:
		return "Backup"
	case About:
		return "About"
	}
	return ""
}

// Permission is one Windows permission row. The original app never prompts at
// launch; each row opens the matching Settings page.
type Permission struct {
	Title    string
	Subtitle string
	URI      string
}

// Permissions lists the capabilities this build actually uses.
func PermissionRows(zh bool) []Permission {
	if zh {
		return []Permission{
			{"辅助功能", "用于把内容粘贴到前台应用。", "ms-settings:privacy-general"},
			{"低级键盘钩子", "片段、双击修饰键和全局快捷键。启动时不提示。", "ms-settings:privacy"},
			{"日历", "未授予日历权限前，会议列表为空。", "ms-settings:privacy-calendar"},
		}
	}
	return []Permission{
		{"UI Automation", "Used to paste into the frontmost app.", "ms-settings:privacy-general"},
		{"Low-level keyboard hook", "Snippets, double-tap and global shortcuts. Not prompted at launch.", "ms-settings:privacy"},
		{"Calendar", "Meetings stay empty until Calendar access is granted.", "ms-settings:privacy-calendar"},
	}
}

// BackupAction identifies one row on the Backup page.
type BackupAction uint8

const (
	BackupExport BackupAction = iota
	BackupImport
	BackupRaycast
	BackupMigrate
)

// BackupActions lists the Backup page rows in the original order.
func BackupActions() []BackupAction {
	return []BackupAction{BackupExport, BackupImport, BackupRaycast, BackupMigrate}
}

// Title is the localized Backup row label.
func (a BackupAction) Title(zh bool) string {
	if zh {
		switch a {
		case BackupExport:
			return "导出设置…"
		case BackupImport:
			return "导入设置…"
		case BackupRaycast:
			return "从 Raycast 导入…"
		default:
			return "迁移旧版数据…"
		}
	}
	switch a {
	case BackupExport:
		return "Export Settings…"
	case BackupImport:
		return "Import Settings…"
	case BackupRaycast:
		return "Import from Raycast…"
	default:
		return "Migrate Legacy Data…"
	}
}

// LaunchAtLoginKey is the HKCU Run value name this build owns.
const LaunchAtLoginKey = "Wincast"
