use crate::app_entry::AppKind;
use crate::i18n::{open_verb, UiLang};
use crate::palette_menu::{
    ActionContext, MenuItem, ID_COPY_PATH, ID_FAVORITE, ID_MOVE_DOWN, ID_MOVE_UP, ID_OPEN, ID_QUIT,
    ID_RESET_RANKING, ID_SHOW_IN_FOLDER, ID_UNINSTALL,
};
use crate::system_action::SystemActionId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Chrome {
    Cancel,
    Actions,
    Language,
    LanguageZh,
    LanguageEn,
    TraySettings,
    TrayQuit,
    AddFavorite,
    RemoveFavorite,
    MoveFavUp,
    MoveFavDown,
    ResetRanking,
    ShowInFolder,
    CopyPath,
    QuitApplication,
    UninstallApplication,
    Open,
}

pub fn chrome(key: Chrome, lang: UiLang) -> &'static str {
    match (key, lang) {
        (Chrome::Cancel, UiLang::En) => "Cancel",
        (Chrome::Cancel, UiLang::ZhHans) => "取消",
        (Chrome::Actions, UiLang::En) => "Actions",
        (Chrome::Actions, UiLang::ZhHans) => "操作",
        (Chrome::Language, UiLang::En) => "Language",
        (Chrome::Language, UiLang::ZhHans) => "语言",
        (Chrome::LanguageZh, _) => "简体中文",
        (Chrome::LanguageEn, _) => "English",
        (Chrome::TraySettings, UiLang::En) => "Settings",
        (Chrome::TraySettings, UiLang::ZhHans) => "设置",
        (Chrome::TrayQuit, UiLang::En) => "Quit Tinycast",
        (Chrome::TrayQuit, UiLang::ZhHans) => "退出 Tinycast",
        (Chrome::AddFavorite, UiLang::En) => "Add to Favorites",
        (Chrome::AddFavorite, UiLang::ZhHans) => "添加到收藏",
        (Chrome::RemoveFavorite, UiLang::En) => "Remove from Favorites",
        (Chrome::RemoveFavorite, UiLang::ZhHans) => "从收藏移除",
        (Chrome::MoveFavUp, UiLang::En) => "Move Favorite Up",
        (Chrome::MoveFavUp, UiLang::ZhHans) => "收藏上移",
        (Chrome::MoveFavDown, UiLang::En) => "Move Favorite Down",
        (Chrome::MoveFavDown, UiLang::ZhHans) => "收藏下移",
        (Chrome::ResetRanking, UiLang::En) => "Reset Ranking",
        (Chrome::ResetRanking, UiLang::ZhHans) => "重置排序",
        (Chrome::ShowInFolder, UiLang::En) => "Show in Folder",
        (Chrome::ShowInFolder, UiLang::ZhHans) => "在文件夹中显示",
        (Chrome::CopyPath, UiLang::En) => "Copy Path",
        (Chrome::CopyPath, UiLang::ZhHans) => "复制路径",
        (Chrome::QuitApplication, UiLang::En) => "Quit Application",
        (Chrome::QuitApplication, UiLang::ZhHans) => "退出应用",
        (Chrome::UninstallApplication, UiLang::En) => "Uninstall Application",
        (Chrome::UninstallApplication, UiLang::ZhHans) => "卸载应用",
        (Chrome::Open, UiLang::En) => "Open",
        (Chrome::Open, UiLang::ZhHans) => "打开",
    }
}

pub fn system_confirm(id: SystemActionId, lang: UiLang) -> Option<(&'static str, &'static str)> {
    let en = match id {
        SystemActionId::Restart => Some((
            "Restart your PC?",
            "Applications with unsaved changes may ask you to save.",
        )),
        SystemActionId::ShutDown => Some((
            "Shut down your PC?",
            "Applications with unsaved changes may ask you to save.",
        )),
        SystemActionId::LogOut => Some((
            "Log out now?",
            "Applications with unsaved changes may ask you to save.",
        )),
        SystemActionId::EmptyTrash => Some((
            "Empty Trash?",
            "The items in the Trash will be permanently deleted.",
        )),
        _ => None,
    }?;
    match lang {
        UiLang::En => Some(en),
        UiLang::ZhHans => match id {
            SystemActionId::Restart => {
                Some(("确定要重启电脑吗？", "未保存的应用可能会提示你保存。"))
            }
            SystemActionId::ShutDown => Some(("确定要关机吗？", "未保存的应用可能会提示你保存。")),
            SystemActionId::LogOut => Some(("确定要注销吗？", "未保存的应用可能会提示你保存。")),
            SystemActionId::EmptyTrash => {
                Some(("确定清空回收站吗？", "回收站中的项目将被永久删除。"))
            }
            _ => None,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralSection {
    GlobalShortcuts,
    Search,
    HyperKey,
    Appearance,
    General,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralRow {
    PaletteRecorder,
    ResetRanking,
    HyperKey,
    HyperShift,
    Appearance,
    Language,
    Compact,
    FavoritesInCompact,
    FollowCursor,
    Draggable,
    LaunchAtLogin,
    ShowInMenuBar,
    PopToRoot,
    AutoSwitchInput,
}

pub fn general_section_title(section: GeneralSection, lang: UiLang) -> &'static str {
    match (section, lang) {
        (GeneralSection::GlobalShortcuts, UiLang::En) => "Global Shortcuts",
        (GeneralSection::GlobalShortcuts, UiLang::ZhHans) => "全局快捷键",
        (GeneralSection::Search, UiLang::En) => "Search",
        (GeneralSection::Search, UiLang::ZhHans) => "搜索",
        (GeneralSection::HyperKey, UiLang::En) => "Hyper Key",
        (GeneralSection::HyperKey, UiLang::ZhHans) => "Hyper 键",
        (GeneralSection::Appearance, UiLang::En) => "Appearance",
        (GeneralSection::Appearance, UiLang::ZhHans) => "外观",
        (GeneralSection::General, UiLang::En) => "General",
        (GeneralSection::General, UiLang::ZhHans) => "通用",
    }
}

pub fn general_section_footer(section: GeneralSection, lang: UiLang) -> Option<&'static str> {
    match (section, lang) {
        (GeneralSection::GlobalShortcuts, UiLang::En) => {
            Some("Summon the fuzzy app launcher.")
        }
        (GeneralSection::GlobalShortcuts, UiLang::ZhHans) => {
            Some("从任意位置呼出模糊搜索启动器。")
        }
        (GeneralSection::Search, UiLang::En) => Some(
            "Tinycast privately learns which results you choose for each query. Reset all learned choices to restore the default order.",
        ),
        (GeneralSection::Search, UiLang::ZhHans) => {
            Some("Tinycast 会私下学习你对每个查询的选择。重置后恢复默认排序。")
        }
        _ => None,
    }
}

pub fn general_row_title(row: GeneralRow, lang: UiLang) -> &'static str {
    match (row, lang) {
        (GeneralRow::PaletteRecorder, UiLang::En) => "App Launcher",
        (GeneralRow::PaletteRecorder, UiLang::ZhHans) => "应用启动器",
        (GeneralRow::ResetRanking, UiLang::En) => "Learned ranking",
        (GeneralRow::ResetRanking, UiLang::ZhHans) => "学习排序",
        (GeneralRow::HyperKey, UiLang::En) => "Hyper Key",
        (GeneralRow::HyperKey, UiLang::ZhHans) => "Hyper 键",
        (GeneralRow::HyperShift, UiLang::En) => "Include Shift",
        (GeneralRow::HyperShift, UiLang::ZhHans) => "包含 Shift",
        (GeneralRow::Appearance, UiLang::En) => "Theme",
        (GeneralRow::Appearance, UiLang::ZhHans) => "主题",
        (GeneralRow::Language, UiLang::En) => chrome(Chrome::Language, UiLang::En),
        (GeneralRow::Language, UiLang::ZhHans) => chrome(Chrome::Language, UiLang::ZhHans),
        (GeneralRow::Compact, UiLang::En) => "Compact mode",
        (GeneralRow::Compact, UiLang::ZhHans) => "紧凑模式",
        (GeneralRow::FavoritesInCompact, UiLang::En) => "Show favorites in compact mode",
        (GeneralRow::FavoritesInCompact, UiLang::ZhHans) => "在紧凑模式显示收藏",
        (GeneralRow::FollowCursor, UiLang::En) => "Follow the cursor",
        (GeneralRow::FollowCursor, UiLang::ZhHans) => "跟随指针",
        (GeneralRow::Draggable, UiLang::En) => "Drag to reposition",
        (GeneralRow::Draggable, UiLang::ZhHans) => "拖动以重新放置",
        (GeneralRow::LaunchAtLogin, UiLang::En) => "Launch at login",
        (GeneralRow::LaunchAtLogin, UiLang::ZhHans) => "登录时启动",
        (GeneralRow::ShowInMenuBar, UiLang::En) => "Show in menu bar",
        (GeneralRow::ShowInMenuBar, UiLang::ZhHans) => "在托盘显示图标",
        (GeneralRow::PopToRoot, UiLang::En) => "Pop to Root",
        (GeneralRow::PopToRoot, UiLang::ZhHans) => "返回根视图",
        (GeneralRow::AutoSwitchInput, UiLang::En) => "Auto-switch input source",
        (GeneralRow::AutoSwitchInput, UiLang::ZhHans) => "自动切换输入法",
    }
}

pub fn general_row_subtitle(row: GeneralRow, lang: UiLang) -> Option<&'static str> {
    match (row, lang) {
        (GeneralRow::PaletteRecorder, UiLang::En) => {
            Some("Toggle palette recorder — click to rebind.")
        }
        (GeneralRow::PaletteRecorder, UiLang::ZhHans) => Some("点击重新绑定呼出快捷键。"),
        (GeneralRow::HyperShift, UiLang::En) => {
            Some("Hyper Key will remap with Shift in the chord.")
        }
        (GeneralRow::HyperShift, UiLang::ZhHans) => Some("组合中会包含 Shift。"),
        (GeneralRow::Appearance, UiLang::En) => Some("Match the system, or pin Light or Dark."),
        (GeneralRow::Appearance, UiLang::ZhHans) => Some("跟随系统，或固定浅色 / 深色。"),
        (GeneralRow::Compact, UiLang::En) => Some("Open the launcher as a slim search bar."),
        (GeneralRow::Compact, UiLang::ZhHans) => Some("以纤细搜索条打开启动器。"),
        (GeneralRow::FavoritesInCompact, UiLang::En) => {
            Some("Pin favorite app icons to the compact bar.")
        }
        (GeneralRow::FavoritesInCompact, UiLang::ZhHans) => Some("把收藏应用图标钉在紧凑条上。"),
        (GeneralRow::FollowCursor, UiLang::En) => {
            Some("Open the launcher on the pointer’s display.")
        }
        (GeneralRow::FollowCursor, UiLang::ZhHans) => Some("在指针所在显示器打开启动器。"),
        (GeneralRow::Draggable, UiLang::En) => {
            Some("Grab the strip above search to move the launcher.")
        }
        (GeneralRow::Draggable, UiLang::ZhHans) => Some("拖动搜索框上方的细条移动启动器。"),
        (GeneralRow::LaunchAtLogin, UiLang::En) => {
            Some("Start Tinycast automatically when you log in.")
        }
        (GeneralRow::LaunchAtLogin, UiLang::ZhHans) => Some("登录 Windows 后自动启动 Tinycast。"),
        (GeneralRow::ShowInMenuBar, UiLang::En) => {
            Some("Keep the Tinycast icon in the menu bar. Shortcuts still work when hidden.")
        }
        (GeneralRow::ShowInMenuBar, UiLang::ZhHans) => {
            Some("在通知区域保留 Tinycast 图标。隐藏后快捷键仍可用。")
        }
        (GeneralRow::AutoSwitchInput, UiLang::En) => Some("Switch IME when the palette opens."),
        (GeneralRow::AutoSwitchInput, UiLang::ZhHans) => Some("打开面板时切换输入法。"),
        _ => None,
    }
}

pub fn general_reset_label(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Reset…",
        UiLang::ZhHans => "重置…",
    }
}

pub fn general_ranking_subtitle(empty: bool, lang: UiLang) -> &'static str {
    match (empty, lang) {
        (true, UiLang::En) => "No learned ranking yet.",
        (true, UiLang::ZhHans) => "还没有学习排序。",
        (false, UiLang::En) => "Clear privately learned result order.",
        (false, UiLang::ZhHans) => "清除私下学习的结果顺序。",
    }
}

pub fn general_appearance_trailing(raw: &str, lang: UiLang) -> &'static str {
    match lang {
        UiLang::ZhHans => match raw {
            "light" => "浅色",
            "dark" => "深色",
            _ => "跟随系统",
        },
        UiLang::En => match raw {
            "light" => "light",
            "dark" => "dark",
            _ => "system",
        },
    }
}

pub fn general_language_trailing(lang: UiLang) -> &'static str {
    match lang {
        UiLang::ZhHans => chrome(Chrome::LanguageZh, lang),
        UiLang::En => chrome(Chrome::LanguageEn, lang),
    }
}

pub fn general_pop_to_root_subtitle(seconds: i64, lang: UiLang) -> String {
    match lang {
        UiLang::En => format!("{seconds} s idle timeout (0 = never)."),
        UiLang::ZhHans => format!("空闲 {seconds} 秒后返回（0 = 从不）。"),
    }
}

pub fn general_pop_to_root_trailing(seconds: i64, lang: UiLang) -> String {
    match (seconds, lang) {
        (0, UiLang::En) => "Never".to_string(),
        (0, UiLang::ZhHans) => "从不".to_string(),
        (s, UiLang::En) => format!("{s}s"),
        (s, UiLang::ZhHans) => format!("{s} 秒"),
    }
}

pub fn general_hyper_off(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Off",
        UiLang::ZhHans => "关闭",
    }
}

pub fn general_hyper_caps_subtitle(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Caps Lock. Takes effect after logoff; cleared on quit.",
        UiLang::ZhHans => "Caps Lock。注销后生效；退出时清除。",
    }
}

pub fn launcher_enable_subtitle(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => {
            "Off hides them all and stops their shortcuts. Uncheck one below to hide just that one."
        }
        UiLang::ZhHans => "关闭后全部隐藏并停止其快捷键。取消勾选某一项只隐藏那一项。",
    }
}

pub fn launcher_search_header(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Search",
        UiLang::ZhHans => "搜索",
    }
}

pub fn launcher_learned_ranking_title(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Learned ranking",
        UiLang::ZhHans => "学习排序",
    }
}

pub fn launcher_reset_button(lang: UiLang) -> &'static str {
    general_reset_label(lang)
}

pub fn launcher_reset_footer(lang: UiLang) -> &'static str {
    general_section_footer(GeneralSection::Search, lang).unwrap_or("")
}

pub fn launcher_reset_confirm_title(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Reset learned launcher ranking?",
        UiLang::ZhHans => "重置已学习的启动器排序？",
    }
}

pub fn launcher_reset_confirm_message(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Tinycast will relearn your preferred results as you use the launcher.",
        UiLang::ZhHans => "你继续使用启动器时，Tinycast 会重新学习偏好结果。",
    }
}

pub fn show_in_launcher(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Show in launcher",
        UiLang::ZhHans => "在启动器中显示",
    }
}

pub fn enable_named(name: &str, lang: UiLang) -> String {
    match lang {
        UiLang::En => format!("Enable {name}"),
        UiLang::ZhHans => format!("启用{name}"),
    }
}

pub fn add_alias_placeholder(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Add Alias",
        UiLang::ZhHans => "添加别名",
    }
}

pub fn record_label(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Record",
        UiLang::ZhHans => "录制",
    }
}

pub fn listening_label(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Listening…",
        UiLang::ZhHans => "正在聆听…",
    }
}

pub fn recording_label(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Recording…",
        UiLang::ZhHans => "正在录制…",
    }
}

pub fn empty_list_copy(query: &str, lang: UiLang) -> String {
    if query.is_empty() {
        match lang {
            UiLang::En => "Nothing here yet.".into(),
            UiLang::ZhHans => "这里还没有内容。".into(),
        }
    } else {
        match lang {
            UiLang::En => format!("No matches for “{query}”."),
            UiLang::ZhHans => format!("没有与“{query}”匹配的结果。"),
        }
    }
}

pub fn launcher_search_prompt(kind: AppKind, lang: UiLang) -> &'static str {
    match (kind, lang) {
        (AppKind::Application, UiLang::En) => "Search applications…",
        (AppKind::Application, UiLang::ZhHans) => "搜索应用…",
        (AppKind::SystemSettings, UiLang::En) => "Search System Settings…",
        (AppKind::SystemSettings, UiLang::ZhHans) => "搜索系统设置…",
        (AppKind::Command, UiLang::En) => "Search commands…",
        (AppKind::Command, UiLang::ZhHans) => "搜索命令…",
        (AppKind::SystemAction, UiLang::En) => "Search system actions…",
        (AppKind::SystemAction, UiLang::ZhHans) => "搜索系统操作…",
        _ => match lang {
            UiLang::En => "Search…",
            UiLang::ZhHans => "搜索…",
        },
    }
}

pub fn remove_label(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Remove",
        UiLang::ZhHans => "移除",
    }
}

pub fn open_settings_action(lang: UiLang) -> &'static str {
    match lang {
        UiLang::En => "Open Settings",
        UiLang::ZhHans => "打开设置",
    }
}

pub fn actions_for_lang(ctx: ActionContext, lang: UiLang) -> Vec<MenuItem> {
    fn item(id: &'static str, label: &'static str, shortcut: Option<&'static str>) -> MenuItem {
        MenuItem {
            id,
            label: label.to_string(),
            shortcut,
        }
    }

    let mut items = vec![item(ID_OPEN, open_verb(ctx.kind, lang), Some("↵"))];
    items.push(item(
        ID_FAVORITE,
        chrome(
            if ctx.is_favorite {
                Chrome::RemoveFavorite
            } else {
                Chrome::AddFavorite
            },
            lang,
        ),
        Some("Ctrl+Shift+F"),
    ));
    if ctx.can_move_up {
        items.push(item(
            ID_MOVE_UP,
            chrome(Chrome::MoveFavUp, lang),
            Some("Ctrl+Alt+Up"),
        ));
    }
    if ctx.can_move_down {
        items.push(item(
            ID_MOVE_DOWN,
            chrome(Chrome::MoveFavDown, lang),
            Some("Ctrl+Alt+Down"),
        ));
    }
    if ctx.has_ranking {
        items.push(item(
            ID_RESET_RANKING,
            chrome(Chrome::ResetRanking, lang),
            None,
        ));
    }
    if ctx.kind.can_reveal_in_folder() {
        items.push(item(
            ID_SHOW_IN_FOLDER,
            chrome(Chrome::ShowInFolder, lang),
            Some("Ctrl+Enter"),
        ));
    }
    if matches!(ctx.kind, AppKind::Application | AppKind::SystemSettings) {
        items.push(item(
            ID_COPY_PATH,
            chrome(Chrome::CopyPath, lang),
            Some("Ctrl+Alt+C"),
        ));
    }
    if ctx.running && ctx.kind == AppKind::Application {
        items.push(item(
            ID_QUIT,
            chrome(Chrome::QuitApplication, lang),
            Some("Ctrl+Shift+Q"),
        ));
    }
    if ctx.kind == AppKind::Application {
        items.push(item(
            ID_UNINSTALL,
            chrome(Chrome::UninstallApplication, lang),
            None,
        ));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_entry::AppKind;
    use crate::i18n::UiLang;
    use crate::palette_menu::ActionContext;

    #[test]
    fn actions_menu_zh_uses_open_verb() {
        let items = actions_for_lang(
            ActionContext::for_kind(AppKind::Application),
            UiLang::ZhHans,
        );
        assert_eq!(items[0].label, "打开应用");
        assert!(items.iter().any(|i| i.label == "添加到收藏"));
        let en = crate::palette_menu::actions_for(ActionContext::for_kind(AppKind::Application));
        assert_eq!(en[0].label, "Open Application");
    }

    #[test]
    fn chrome_cancel_zh() {
        assert_eq!(chrome(Chrome::Cancel, UiLang::ZhHans), "取消");
        assert_eq!(chrome(Chrome::Cancel, UiLang::En), "Cancel");
    }

    #[test]
    fn general_show_in_menu_bar_zh() {
        assert_eq!(
            general_row_title(GeneralRow::ShowInMenuBar, UiLang::ZhHans),
            "在托盘显示图标"
        );
    }

    #[test]
    fn pop_to_root_trailing_stays_short() {
        assert_eq!(general_pop_to_root_trailing(0, UiLang::En), "Never");
        assert_eq!(general_pop_to_root_trailing(0, UiLang::ZhHans), "从不");
        assert_eq!(general_pop_to_root_trailing(10, UiLang::En), "10s");
        assert!(general_pop_to_root_trailing(30, UiLang::ZhHans).len() < 12);
        let long = general_pop_to_root_subtitle(0, UiLang::ZhHans);
        assert!(long.chars().count() > 8);
    }

    #[test]
    fn launcher_item_copy_zh() {
        assert_ne!(
            launcher_enable_subtitle(UiLang::ZhHans),
            launcher_enable_subtitle(UiLang::En)
        );
    }

    #[test]
    fn system_confirm_zh_is_not_english() {
        let zh = system_confirm(SystemActionId::Restart, UiLang::ZhHans).unwrap();
        let en = system_confirm(SystemActionId::Restart, UiLang::En).unwrap();
        assert_eq!(en.0, "Restart your PC?");
        assert_eq!(zh.0, "确定要重启电脑吗？");
    }
}
