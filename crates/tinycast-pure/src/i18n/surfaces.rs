use crate::i18n::UiLang;

fn pick<'a>(lang: UiLang, en: &'a str, zh: &'a str) -> &'a str {
    match lang {
        UiLang::En => en,
        UiLang::ZhHans => zh,
    }
}

pub fn onboarding_title(step: usize, lang: UiLang) -> &'static str {
    match step {
        0 => pick(lang, "Welcome to Tinycast", "欢迎使用 Tinycast"),
        1 => pick(lang, "Enable Pasting", "允许粘贴"),
        2 => pick(lang, "Import from Raycast", "从 Raycast 导入"),
        _ => pick(lang, "You're all set", "一切就绪"),
    }
}

pub fn onboarding_subtitle(step: usize, lang: UiLang) -> &'static str {
    match step {
        0 => pick(
            lang,
            "Set a shortcut to summon the launcher from anywhere.",
            "设置快捷键，从任意位置呼出启动器。",
        ),
        1 => pick(
            lang,
            "Let Tinycast paste items back into the app you were using.",
            "允许 Tinycast 把内容粘贴回你刚才使用的应用。",
        ),
        2 => pick(
            lang,
            "Bring your shortcuts, favorites, and clipboard history along.",
            "带上你的快捷键、收藏和剪贴板历史。",
        ),
        _ => pick(
            lang,
            "Tinycast is ready. Press your shortcut anytime to start.",
            "Tinycast 已就绪。随时按快捷键开始。",
        ),
    }
}

pub fn onboarding_continue(lang: UiLang) -> &'static str {
    pick(lang, "Continue", "继续")
}

pub fn onboarding_get_started(lang: UiLang) -> &'static str {
    pick(lang, "Get Started", "开始使用")
}

pub fn onboarding_record_shortcut(lang: UiLang) -> &'static str {
    pick(lang, "Record shortcut", "录制快捷键")
}

pub fn dialog_set_volume(lang: UiLang) -> &'static str {
    pick(lang, "Set Volume", "设置音量")
}

pub fn dialog_set_volume_message(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Choose the output volume. Use Left and Right to adjust.",
        "选择输出音量。用左右方向键调节。",
    )
}

pub fn qa_replace(lang: UiLang) -> &'static str {
    pick(lang, "Replace", "替换")
}

pub fn qa_copy(lang: UiLang) -> &'static str {
    pick(lang, "Copy", "复制")
}

pub fn notes_window_title(lang: UiLang) -> &'static str {
    pick(lang, "Notes", "笔记")
}

pub fn notes_start_writing(lang: UiLang) -> &'static str {
    pick(lang, "Start writing…", "开始书写…")
}

pub fn notes_create(lang: UiLang) -> &'static str {
    pick(lang, "Create", "新建")
}

pub fn notes_browse(lang: UiLang) -> &'static str {
    pick(lang, "Browse", "浏览")
}

pub fn notes_folder(lang: UiLang) -> &'static str {
    pick(lang, "Folder", "文件夹")
}

pub fn support_title(lang: UiLang) -> &'static str {
    pick(lang, "Support Tinycast", "支持 Tinycast")
}

pub fn about_window_title(lang: UiLang) -> &'static str {
    pick(lang, "About Tinycast", "关于 Tinycast")
}

pub fn support_built_with_love(lang: UiLang) -> &'static str {
    pick(lang, "Built with love.", "用心打造。")
}

pub fn support_keeps_independent(lang: UiLang) -> &'static str {
    pick(
        lang,
        "A small launcher. Support keeps it independent.",
        "一个小启动器。你的支持让它保持独立。",
    )
}

pub fn support_checkout(lang: UiLang) -> &'static str {
    pick(lang, "Secure checkout on Polar.", "在 Polar 安全结账。")
}

pub fn support_remind_later(lang: UiLang) -> &'static str {
    pick(lang, "Remind me later", "稍后提醒")
}

pub fn update_up_to_date(lang: UiLang) -> &'static str {
    pick(lang, "You're up to date.", "已是最新版本。")
}

pub fn update_check_failed(lang: UiLang) -> &'static str {
    pick(lang, "The update check failed.", "检查更新失败。")
}

pub fn update_none_configured(lang: UiLang) -> &'static str {
    pick(lang, "No updates configured.", "未配置更新。")
}

pub fn empty_no_apps(lang: UiLang) -> &'static str {
    pick(lang, "No apps found", "未找到应用")
}

pub fn empty_no_calculations(lang: UiLang) -> &'static str {
    pick(lang, "No calculations yet", "还没有计算记录")
}

pub fn empty_no_matching_calculations(lang: UiLang) -> &'static str {
    pick(lang, "No matching calculations", "没有匹配的计算")
}

pub fn empty_loading_emoji(lang: UiLang) -> &'static str {
    pick(lang, "Loading emoji…", "正在加载表情…")
}

pub fn empty_no_emoji(lang: UiLang) -> &'static str {
    pick(lang, "No emoji found", "未找到表情")
}

pub fn empty_no_meetings(lang: UiLang) -> &'static str {
    pick(lang, "No matching meetings", "没有匹配的会议")
}

pub fn empty_nothing_scheduled(lang: UiLang) -> &'static str {
    pick(lang, "Nothing scheduled today or tomorrow", "今天和明天没有日程")
}

pub fn footer_copy_answer(lang: UiLang) -> &'static str {
    pick(lang, "Copy Answer", "复制答案")
}

pub fn footer_open(lang: UiLang) -> &'static str {
    pick(lang, "Open", "打开")
}

pub fn footer_paste(lang: UiLang) -> &'static str {
    pick(lang, "Paste", "粘贴")
}

pub fn footer_continue(lang: UiLang) -> &'static str {
    pick(lang, "Continue", "继续")
}

pub fn footer_stop(lang: UiLang) -> &'static str {
    pick(lang, "Stop", "停止")
}

pub fn footer_send(lang: UiLang) -> &'static str {
    pick(lang, "Send", "发送")
}

pub fn join_now_title(title: &str, lang: UiLang) -> String {
    match lang {
        UiLang::En => format!("Join {title}?"),
        UiLang::ZhHans => format!("加入 {title}？"),
    }
}

pub fn join_now_message(lang: UiLang) -> &'static str {
    pick(lang, "This meeting is starting now.", "这场会议即将开始。")
}

pub fn join_now_accept(lang: UiLang) -> &'static str {
    pick(lang, "Join", "加入")
}

pub fn clipboard_filter_title(filter: &str, lang: UiLang) -> &'static str {
    match (filter, lang) {
        ("all", UiLang::ZhHans) => "全部类型",
        ("text", UiLang::ZhHans) => "仅文本",
        ("images", UiLang::ZhHans) => "仅图片",
        ("links", UiLang::ZhHans) => "仅链接",
        ("emails", UiLang::ZhHans) => "仅邮件",
        ("all", _) => "All Types",
        ("text", _) => "Text Only",
        ("images", _) => "Images Only",
        ("links", _) => "Links Only",
        _ => pick(lang, "Emails Only", "仅邮件"),
    }
}

pub fn clipboard_empty(filter: &str, lang: UiLang) -> &'static str {
    match (filter, lang) {
        ("all", UiLang::ZhHans) => "剪贴板历史为空",
        ("text", UiLang::ZhHans) => "剪贴板历史中没有文本",
        ("images", UiLang::ZhHans) => "剪贴板历史中没有图片",
        ("links", UiLang::ZhHans) => "剪贴板历史中没有链接",
        ("emails", UiLang::ZhHans) => "剪贴板历史中没有邮件",
        ("all", _) => "Clipboard history is empty",
        ("text", _) => "No text in clipboard history",
        ("images", _) => "No images in clipboard history",
        ("links", _) => "No links in clipboard history",
        _ => pick(
            lang,
            "No email addresses in clipboard history",
            "剪贴板历史中没有邮件",
        ),
    }
}

pub fn empty_nothing_left(lang: UiLang) -> &'static str {
    pick(lang, "Nothing left to remove", "没有可移除的项目")
}

pub fn empty_no_matching_files(lang: UiLang) -> &'static str {
    pick(lang, "No matching files", "没有匹配的文件")
}

pub fn empty_no_quicklinks(lang: UiLang) -> &'static str {
    pick(lang, "No quicklinks yet", "还没有快捷链接")
}

pub fn empty_no_matching_quicklinks(lang: UiLang) -> &'static str {
    pick(lang, "No matching quicklinks", "没有匹配的快捷链接")
}

pub fn empty_no_chats(lang: UiLang) -> &'static str {
    pick(lang, "No chats yet", "还没有对话")
}

pub fn empty_no_matching_chats(lang: UiLang) -> &'static str {
    pick(lang, "No matching chats", "没有匹配的对话")
}

pub fn uninstall_label(lang: UiLang) -> &'static str {
    pick(lang, "Uninstall", "卸载")
}

pub fn ai_new_chat(lang: UiLang) -> &'static str {
    pick(lang, "New Chat", "新对话")
}

pub fn ai_chat_history(lang: UiLang) -> &'static str {
    pick(lang, "Chat History", "对话历史")
}

pub fn ai_settings_action(lang: UiLang) -> &'static str {
    pick(lang, "AI Settings", "AI 设置")
}

pub fn ai_stop_response(lang: UiLang) -> &'static str {
    pick(lang, "Stop Response", "停止回复")
}

pub fn ai_copy_last_response(lang: UiLang) -> &'static str {
    pick(lang, "Copy Last Response", "复制上次回复")
}

pub fn qa_working(lang: UiLang) -> &'static str {
    pick(lang, "Working…", "正在处理…")
}

pub fn command_failed(lang: UiLang) -> &'static str {
    pick(lang, "Command failed", "命令失败")
}

pub fn stage_manager_unavailable(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Stage Manager is not available on Windows.",
        "Windows 上不提供 Stage Manager。",
    )
}

pub fn quit_all_title(count: usize, lang: UiLang) -> String {
    match lang {
        UiLang::En if count == 1 => "Quit 1 application?".into(),
        UiLang::En => format!("Quit {count} applications?"),
        UiLang::ZhHans if count == 1 => "退出 1 个应用？".into(),
        UiLang::ZhHans => format!("退出 {count} 个应用？"),
    }
}

pub fn quit_all_message(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Applications with unsaved changes will ask you to save.",
        "未保存的应用会提示你保存。",
    )
}

pub fn quit_all_accept(lang: UiLang) -> &'static str {
    pick(lang, "Quit All", "全部退出")
}

pub fn calendar_consent_title(lang: UiLang) -> &'static str {
    pick(lang, "Turn on Calendar?", "启用日历？")
}

pub fn calendar_consent_message(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Tinycast will read your calendar to show upcoming meetings and join links.",
        "Tinycast 会读取日历以显示即将开始的会议和加入链接。",
    )
}

pub fn camera_preview_title(lang: UiLang) -> &'static str {
    pick(lang, "Camera preview", "摄像头预览")
}

pub fn camera_ready(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Camera is ready. Join or cancel.",
        "摄像头已就绪。加入或取消。",
    )
}

pub fn camera_unavailable(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Camera unavailable. You can still join.",
        "摄像头不可用。仍可加入。",
    )
}

pub fn uninstall_confirm_title(lang: UiLang) -> &'static str {
    pick(lang, "Move to Recycle Bin?", "移到回收站？")
}

pub fn uninstall_confirm_message(name: &str, count: usize, lang: UiLang) -> String {
    match lang {
        UiLang::En => format!(
            "Tinycast will move {count} item(s) for “{name}” to the Recycle Bin. Nothing is permanently deleted."
        ),
        UiLang::ZhHans => format!(
            "Tinycast 会把“{name}”的 {count} 项移到回收站。不会永久删除。"
        ),
    }
}

pub fn clipboard_pinned(lang: UiLang) -> &'static str {
    pick(lang, "Pinned", "已固定")
}

pub fn clipboard_image(lang: UiLang) -> &'static str {
    pick(lang, "Image", "图片")
}

pub fn hyper_key_title(raw: &str, lang: UiLang) -> &'static str {
    match (raw, lang) {
        ("none", UiLang::ZhHans) | ("", UiLang::ZhHans) => "关闭",
        ("none", _) | ("", _) => "None",
        ("capsLock", _) => "Caps Lock (⇪)",
        ("rightControl", UiLang::ZhHans) => "右 Ctrl",
        ("rightControl", _) => "Right Control",
        ("rightShift", UiLang::ZhHans) => "右 Shift",
        ("rightShift", _) => "Right Shift",
        ("rightOption", UiLang::ZhHans) => "右 Alt",
        ("rightOption", _) => "Right Alt",
        ("rightCommand", UiLang::ZhHans) => "右 Win",
        ("rightCommand", _) => "Right Win",
        _ => pick(lang, "None", "关闭"),
    }
}

pub fn editor_save(lang: UiLang) -> &'static str {
    pick(lang, "Save", "保存")
}

pub fn editor_name_label(lang: UiLang) -> &'static str {
    pick(lang, "Name", "名称")
}

pub fn editor_needs_confirmation(lang: UiLang) -> &'static str {
    pick(lang, "Needs confirmation", "需要确认")
}

pub fn ignore_pattern_title(lang: UiLang) -> &'static str {
    pick(lang, "Ignore pattern", "忽略规则")
}

pub fn file_search_empty(kind: &str, lang: UiLang) -> &'static str {
    match (kind, lang) {
        ("type", UiLang::ZhHans) => "输入以搜索文件和文件夹",
        ("unavailable", UiLang::ZhHans) => "文件搜索不可用",
        ("none", UiLang::ZhHans) => "未找到文件",
        ("searching", UiLang::ZhHans) => "正在搜索文件…",
        ("type", _) => "Type to search files and folders",
        ("unavailable", _) => "File search is unavailable",
        ("none", _) => "No files found",
        _ => pick(lang, "Searching files…", "正在搜索文件…"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_copy_zh_differs_from_en() {
        assert_eq!(ai_new_chat(UiLang::ZhHans), "新对话");
        assert_eq!(camera_preview_title(UiLang::ZhHans), "摄像头预览");
        assert_eq!(uninstall_confirm_title(UiLang::ZhHans), "移到回收站？");
        assert_eq!(clipboard_pinned(UiLang::ZhHans), "已固定");
        assert_eq!(hyper_key_title("none", UiLang::ZhHans), "关闭");
        assert_eq!(quit_all_accept(UiLang::ZhHans), "全部退出");
    }
}
