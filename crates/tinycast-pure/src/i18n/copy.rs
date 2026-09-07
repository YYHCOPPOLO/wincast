use crate::app_entry::AppKind;
use crate::i18n::{open_verb, UiLang};
use crate::palette_menu::{
    ActionContext, MenuItem, ID_COPY_PATH, ID_FAVORITE, ID_MOVE_DOWN, ID_MOVE_UP, ID_OPEN,
    ID_QUIT, ID_RESET_RANKING, ID_SHOW_IN_FOLDER, ID_UNINSTALL,
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
            SystemActionId::Restart => Some((
                "确定要重启电脑吗？",
                "未保存的应用可能会提示你保存。",
            )),
            SystemActionId::ShutDown => Some((
                "确定要关机吗？",
                "未保存的应用可能会提示你保存。",
            )),
            SystemActionId::LogOut => Some((
                "确定要注销吗？",
                "未保存的应用可能会提示你保存。",
            )),
            SystemActionId::EmptyTrash => Some((
                "确定清空回收站吗？",
                "回收站中的项目将被永久删除。",
            )),
            _ => None,
        },
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
        let items = actions_for_lang(ActionContext::for_kind(AppKind::Application), UiLang::ZhHans);
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
}
