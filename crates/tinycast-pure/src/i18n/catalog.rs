use crate::app_entry::AppKind;
use crate::command_id::CommandID;
use crate::i18n::UiLang;
use crate::palette_mode::PaletteMode;
use crate::settings_tab::{SettingsSection, SettingsTab};

pub fn command_title(id: CommandID, lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => id.name(),
        UiLang::ZhHans => match id {
            CommandID::AiChat => "AI 对话",
            CommandID::FixGrammar => "修正语法",
            CommandID::Rewrite => "改写",
            CommandID::Translate => "翻译",
            CommandID::Summarize => "摘要",
            CommandID::CalculatorHistory => "计算器历史",
            CommandID::ClipboardHistory => "剪贴板历史",
            CommandID::SearchEmoji => "搜索表情与符号",
            CommandID::SearchFiles => "搜索文件",
            CommandID::JoinNextMeeting => "加入下一场会议",
            CommandID::CopyMeetingLink => "复制会议链接",
            CommandID::MySchedule => "我的日程",
            CommandID::OpenInCalendar => "在日历中打开",
            CommandID::CreateEvent => "创建日程",
            CommandID::ShowNotes => "显示笔记",
            CommandID::CreateNote => "新建笔记",
            CommandID::SearchNotes => "搜索笔记",
            CommandID::CreateQuicklink => "新建快捷链接",
            CommandID::SearchQuicklinks => "搜索快捷链接",
            CommandID::ImportQuicklinks => "导入快捷链接",
            CommandID::ExportQuicklinks => "导出快捷链接",
            CommandID::ExportSettings => "导出设置",
            CommandID::ImportSettings => "导入设置",
            CommandID::ImportFromRaycast => "从 Raycast 导入",
            CommandID::CheckForUpdates => "检查更新",
            CommandID::Settings => "设置",
            CommandID::About => "关于 Tinycast",
            CommandID::Support => "支持 Tinycast",
            CommandID::Quit => "退出 Tinycast",
        },
    }
}

pub fn settings_tab_title(tab: SettingsTab, lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => tab.title(),
        UiLang::ZhHans => match tab {
            SettingsTab::General => "通用",
            SettingsTab::Permissions => "权限",
            SettingsTab::Applications => "应用",
            SettingsTab::SystemSettings => "系统设置",
            SettingsTab::SystemActions => "系统操作",
            SettingsTab::Commands => "命令",
            SettingsTab::Quicklinks => "快捷链接",
            SettingsTab::Ai => "AI",
            SettingsTab::QuickActions => "快捷操作",
            SettingsTab::FileSearch => "文件搜索",
            SettingsTab::Notes => "笔记",
            SettingsTab::Snippets => "片段",
            SettingsTab::WindowManagement => "窗口管理",
            SettingsTab::Clipboard => "剪贴板",
            SettingsTab::Emoji => "表情与符号",
            SettingsTab::Calendar => "日历",
            SettingsTab::Extensions => "扩展",
            SettingsTab::Backup => "备份",
            SettingsTab::About => "关于",
        },
    }
}

pub fn settings_section_title(section: SettingsSection, lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => section.title(),
        UiLang::ZhHans => match section {
            SettingsSection::General => "通用",
            SettingsSection::Launcher => "启动器",
            SettingsSection::Features => "功能",
            SettingsSection::Advanced => "高级",
        },
    }
}

pub fn palette_placeholder(mode: PaletteMode, lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => mode.placeholder(),
        UiLang::ZhHans => match mode {
            PaletteMode::Launcher => "搜索应用和命令…",
            PaletteMode::Clipboard => "输入以筛选条目…",
            PaletteMode::Ai => "问我任何问题…",
            PaletteMode::AiHistory => "搜索对话…",
            PaletteMode::CalculatorHistory => "计算、换算单位，或搜索历史计算…",
            PaletteMode::Emoji => "搜索表情与符号…",
            PaletteMode::FileSearch => "搜索文件和文件夹…",
            PaletteMode::Schedule => "搜索今天和明天…",
            PaletteMode::Uninstall => "按名称筛选文件和文件夹…",
            PaletteMode::Quicklinks => "搜索快捷链接…",
            PaletteMode::QuicklinkArguments => "输入值…",
            PaletteMode::ExtensionCommand => "搜索…",
        },
    }
}

pub fn kind_section_title(kind: AppKind, lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => kind.section_title(),
        UiLang::ZhHans => match kind {
            AppKind::Application => "应用",
            AppKind::SystemSettings => "系统设置",
            AppKind::Quicklink => "快捷链接",
            AppKind::Snippet => "片段",
            AppKind::SystemAction => "系统操作",
            AppKind::WindowCommand => "窗口管理",
            AppKind::CustomCommand => "自定义命令",
            AppKind::Command => "命令",
        },
    }
}

pub fn kind_label(kind: AppKind, lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => kind.kind_label(),
        UiLang::ZhHans => match kind {
            AppKind::Application => "应用",
            AppKind::SystemSettings => "系统设置",
            AppKind::Quicklink => "快捷链接",
            AppKind::Snippet => "片段",
            AppKind::SystemAction => "系统操作",
            AppKind::WindowCommand => "窗口管理",
            AppKind::CustomCommand => "自定义命令",
            AppKind::Command => "命令",
        },
    }
}

pub fn open_verb(kind: AppKind, lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => kind.open_verb(),
        UiLang::ZhHans => match kind {
            AppKind::Application => "打开应用",
            AppKind::SystemSettings => "打开系统设置",
            AppKind::Quicklink => "打开快捷链接",
            AppKind::Snippet => "粘贴片段",
            AppKind::SystemAction => "运行系统操作",
            AppKind::WindowCommand => "移动窗口",
            AppKind::CustomCommand => "运行自定义命令",
            AppKind::Command => "运行命令",
        },
    }
}

pub fn favorites_title(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Favorites",
        UiLang::ZhHans => "收藏",
    }
}

pub fn results_title(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Results",
        UiLang::ZhHans => "结果",
    }
}

pub fn window_command_title(
    id: crate::window_command::WindowCommandId,
    lang: UiLang,
) -> &'static str {
    use crate::window_command::WindowCommandId;
    match lang {
        UiLang::En => id.name(),
        UiLang::ZhHans => match id {
            WindowCommandId::LeftHalf => "左半屏",
            WindowCommandId::RightHalf => "右半屏",
            WindowCommandId::TopHalf => "上半屏",
            WindowCommandId::BottomHalf => "下半屏",
            WindowCommandId::TopLeftQuarter => "左上四分之一",
            WindowCommandId::TopRightQuarter => "右上四分之一",
            WindowCommandId::BottomLeftQuarter => "左下四分之一",
            WindowCommandId::BottomRightQuarter => "右下四分之一",
            WindowCommandId::FirstThreeFourths => "前四分之三",
            WindowCommandId::LastThreeFourths => "后四分之三",
            WindowCommandId::FirstThird => "左三分之一",
            WindowCommandId::CenterThird => "中三分之一",
            WindowCommandId::LastThird => "右三分之一",
            WindowCommandId::FirstTwoThirds => "左三分之二",
            WindowCommandId::LastTwoThirds => "右三分之二",
            WindowCommandId::Maximize => "最大化",
            WindowCommandId::AlmostMaximize => "接近最大化",
            WindowCommandId::ReasonableSize => "合适大小",
            WindowCommandId::MaximizeHeight => "高度最大化",
            WindowCommandId::MaximizeWidth => "宽度最大化",
            WindowCommandId::Center => "居中",
            WindowCommandId::CenterHalf => "居中半屏",
            WindowCommandId::MakeLarger => "放大",
            WindowCommandId::MakeSmaller => "缩小",
            WindowCommandId::Restore => "还原窗口",
            WindowCommandId::MoveLeft => "向左移动",
            WindowCommandId::MoveRight => "向右移动",
            WindowCommandId::MoveUp => "向上移动",
            WindowCommandId::MoveDown => "向下移动",
            WindowCommandId::NextDisplay => "移到下一块显示器",
            WindowCommandId::PreviousDisplay => "移到上一块显示器",
            WindowCommandId::ToggleFullscreen => "切换全屏",
            WindowCommandId::PreviousSpace => "上一个虚拟桌面",
            WindowCommandId::NextSpace => "下一个虚拟桌面",
        },
    }
}

pub fn window_group_title(g: crate::window_command::WindowGroup, lang: UiLang) -> &'static str {
    use crate::window_command::WindowGroup;
    match lang {
        UiLang::En => g.title(),
        UiLang::ZhHans => match g {
            WindowGroup::Halves => "对半",
            WindowGroup::Quarters => "四分",
            WindowGroup::Fourths => "四分之三",
            WindowGroup::Thirds => "三分",
            WindowGroup::Sizing => "尺寸",
            WindowGroup::Moving => "移动",
            WindowGroup::Fullscreen => "全屏",
            WindowGroup::Spaces => "虚拟桌面",
        },
    }
}

pub fn system_action_title(id: crate::system_action::SystemActionId, lang: UiLang) -> &'static str {
    use crate::system_action::SystemActionId;
    match lang {
        UiLang::En => id.name(),
        UiLang::ZhHans => match id {
            SystemActionId::LockScreen => "锁定屏幕",
            SystemActionId::Sleep => "睡眠",
            SystemActionId::SleepDisplays => "关闭显示器",
            SystemActionId::Restart => "重启",
            SystemActionId::ShutDown => "关机",
            SystemActionId::LogOut => "注销",
            SystemActionId::ShowScreenSaver => "显示屏幕保护程序",
            SystemActionId::PlayPause => "播放 / 暂停",
            SystemActionId::NextTrack => "下一首",
            SystemActionId::PreviousTrack => "上一首",
            SystemActionId::ToggleMute => "切换静音",
            SystemActionId::VolumeUp => "提高音量",
            SystemActionId::VolumeDown => "降低音量",
            SystemActionId::SetVolume => "设置音量…",
            SystemActionId::Volume0 => "音量设为 0%",
            SystemActionId::Volume25 => "音量设为 25%",
            SystemActionId::Volume50 => "音量设为 50%",
            SystemActionId::Volume75 => "音量设为 75%",
            SystemActionId::Volume100 => "音量设为 100%",
            SystemActionId::ShowDesktop => "显示桌面",
            SystemActionId::ToggleAppearance => "切换系统外观",
            SystemActionId::ToggleStageManager => "切换 Stage Manager",
            SystemActionId::OpenTrash => "打开回收站",
            SystemActionId::EmptyTrash => "清空回收站",
            SystemActionId::EjectAllDisks => "弹出所有磁盘",
            SystemActionId::ToggleHiddenFiles => "切换隐藏文件",
            SystemActionId::HideOtherApps => "隐藏其他应用",
            SystemActionId::UnhideAllApps => "显示所有隐藏应用",
            SystemActionId::QuitAllApps => "退出所有应用",
            SystemActionId::DismissNotifications => "清除通知",
            SystemActionId::ToggleBluetooth => "开关蓝牙",
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::command_id::CommandID;
    use crate::i18n::{
        command_title, kind_section_title, open_verb, palette_placeholder, settings_section_title,
        settings_tab_title, system_action_title, window_command_title, UiLang,
    };

    #[test]
    fn ui_lang_parse_defaults_to_zh_hans() {
        assert_eq!(UiLang::parse(""), UiLang::ZhHans);
        assert_eq!(UiLang::parse("zh-Hans"), UiLang::ZhHans);
        assert_eq!(UiLang::parse("zh"), UiLang::ZhHans);
        assert_eq!(UiLang::parse("zh-CN"), UiLang::ZhHans);
        assert_eq!(UiLang::parse("nope"), UiLang::ZhHans);
        assert_eq!(UiLang::parse("en"), UiLang::En);
        assert_eq!(UiLang::parse("en-US"), UiLang::En);
        assert_eq!(UiLang::default(), UiLang::ZhHans);
        assert_eq!(UiLang::ZhHans.as_str(), "zh-Hans");
        assert_eq!(UiLang::En.as_str(), "en");
        assert_eq!(UiLang::ZhHans.dwrite_locale(), "zh-CN");
        assert_eq!(UiLang::En.dwrite_locale(), "en-US");
        assert_eq!(UiLang::ZhHans.cycle(), UiLang::En);
        assert_eq!(UiLang::En.cycle(), UiLang::ZhHans);
    }

    #[test]
    fn command_titles_match_spec_and_oracle_name_stays_english() {
        assert_eq!(CommandID::Settings.name(), "Settings");
        assert_eq!(command_title(CommandID::Settings, UiLang::En), "Settings");
        assert_eq!(command_title(CommandID::Settings, UiLang::ZhHans), "设置");
        assert_eq!(command_title(CommandID::AiChat, UiLang::ZhHans), "AI 对话");
        assert_eq!(
            command_title(CommandID::Quit, UiLang::ZhHans),
            "退出 Tinycast"
        );
        assert_eq!(
            command_title(CommandID::SearchEmoji, UiLang::ZhHans),
            "搜索表情与符号"
        );
    }

    #[test]
    fn every_command_has_zh_and_en() {
        for id in CommandID::all() {
            let en = command_title(*id, UiLang::En);
            let zh = command_title(*id, UiLang::ZhHans);
            assert_eq!(en, id.name());
            assert!(!zh.is_empty());
            assert_ne!(zh, en, "{}", id.raw());
        }
    }

    #[test]
    fn settings_tabs_match_spec_11_1() {
        use crate::settings_tab::{SettingsSection, SettingsTab};
        assert_eq!(SettingsTab::General.title(), "General");
        assert_eq!(
            settings_tab_title(SettingsTab::General, UiLang::ZhHans),
            "通用"
        );
        assert_eq!(
            settings_tab_title(SettingsTab::WindowManagement, UiLang::ZhHans),
            "窗口管理"
        );
        assert_eq!(settings_tab_title(SettingsTab::Ai, UiLang::ZhHans), "AI");
        assert_eq!(settings_tab_title(SettingsTab::Ai, UiLang::En), "AI");
        assert_eq!(
            settings_section_title(SettingsSection::Features, UiLang::ZhHans),
            "功能"
        );
    }

    #[test]
    fn placeholders_match_spec_11_3() {
        use crate::palette_mode::PaletteMode;
        assert_eq!(
            PaletteMode::Launcher.placeholder(),
            "Search for apps and commands…"
        );
        assert_eq!(
            palette_placeholder(PaletteMode::Launcher, UiLang::ZhHans),
            "搜索应用和命令…"
        );
        assert_eq!(
            palette_placeholder(PaletteMode::Ai, UiLang::ZhHans),
            "问我任何问题…"
        );
    }

    #[test]
    fn named_by_accepts_chinese_and_english() {
        use crate::app_entry::AppKind;
        assert_eq!(AppKind::named_by("Commands"), Some(AppKind::Command));
        assert_eq!(AppKind::named_by("命令"), Some(AppKind::Command));
        assert_eq!(AppKind::named_by("片段"), Some(AppKind::Snippet));
        assert_eq!(AppKind::named_by("窗口管理"), Some(AppKind::WindowCommand));
        assert_eq!(kind_section_title(AppKind::Command, UiLang::ZhHans), "命令");
        assert_eq!(open_verb(AppKind::Application, UiLang::ZhHans), "打开应用");
        assert_eq!(
            open_verb(AppKind::Application, UiLang::En),
            "Open Application"
        );
    }

    #[test]
    fn settings_command_matches_english_query_in_zh() {
        use crate::search_relevance::score;
        let entry = CommandID::Settings.as_entry_for(UiLang::ZhHans);
        assert_eq!(entry.name, "设置");
        assert!(score("设置", &entry.fields).is_some());
        assert!(score("settings", &entry.fields).is_some());
        assert!(score("Settings", &entry.fields).is_some());
    }

    #[test]
    fn as_entry_stays_english_oracle() {
        let entry = CommandID::Settings.as_entry();
        assert_eq!(entry.name, "Settings");
    }

    #[test]
    fn every_window_command_has_zh() {
        use crate::window_command::WindowCommandId;
        for id in WindowCommandId::ALL {
            let en = window_command_title(*id, UiLang::En);
            let zh = window_command_title(*id, UiLang::ZhHans);
            assert_eq!(en, id.name());
            assert!(!zh.is_empty());
            assert_ne!(zh, en, "{}", id.raw());
        }
    }

    #[test]
    fn every_system_action_has_zh() {
        use crate::system_action::SystemActionId;
        for id in SystemActionId::ALL {
            let en = system_action_title(*id, UiLang::En);
            let zh = system_action_title(*id, UiLang::ZhHans);
            assert_eq!(en, id.name());
            assert!(!zh.is_empty());
            assert_ne!(zh, en, "{}", id.raw());
        }
    }
}
