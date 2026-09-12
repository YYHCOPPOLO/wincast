use crate::i18n::{chrome, settings_tab_title, Chrome, UiLang};
use crate::settings_tab::SettingsTab;

fn pick<'a>(lang: UiLang, en: &'a str, zh: &'a str) -> &'a str {
    match lang {
        UiLang::En => en,
        UiLang::ZhHans => zh,
    }
}

pub fn permission_title(index: usize, lang: UiLang) -> &'static str {
    match index {
        0 => pick(lang, "UI Automation", "辅助功能"),
        1 => pick(lang, "Low-level keyboard hook", "自动化"),
        2 => pick(lang, "Calendar", "日历"),
        3 => pick(lang, "Camera", "摄像头"),
        _ => "",
    }
}

pub fn permission_subtitle(index: usize, lang: UiLang) -> &'static str {
    match index {
        0 => pick(
            lang,
            "Used to paste into the frontmost app.",
            "用于把内容粘贴到前台应用。",
        ),
        1 => pick(
            lang,
            "Snippets, double-tap, and Hyper. Not prompted at launch.",
            "片段、双击和 Hyper。启动时不提示。",
        ),
        2 => pick(
            lang,
            "Meetings stay empty until Calendar access is granted.",
            "未授予日历权限前，会议列表为空。",
        ),
        3 => pick(
            lang,
            "Camera preview for joining meetings.",
            "加入会议时的摄像头预览。",
        ),
        _ => "",
    }
}

pub fn about_product(_lang: UiLang) -> &'static str {
    "Tinycast for Windows"
}

pub fn about_support(lang: UiLang) -> &'static str {
    pick(lang, "Support Tinycast…", "支持 Tinycast…")
}

pub fn backup_export(lang: UiLang) -> &'static str {
    pick(lang, "Export Settings…", "导出设置…")
}

pub fn backup_import(lang: UiLang) -> &'static str {
    pick(lang, "Import Settings…", "导入设置…")
}

pub fn backup_raycast(lang: UiLang) -> &'static str {
    pick(lang, "Import from Raycast…", "从 Raycast 导入…")
}

pub fn ai_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Enable AI", "启用 AI")
}

pub fn ai_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Chat with the model you choose; nothing is loaded or sent until it is on.",
        "选择模型后开始对话；关闭时不加载也不发送。",
    )
}

pub fn ai_web_search_title(lang: UiLang) -> &'static str {
    pick(lang, "Web search", "网络搜索")
}

pub fn ai_web_search_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Let OpenRouter models search the web.",
        "允许 OpenRouter 模型搜索网络。",
    )
}

pub fn ai_system_prompt_title(lang: UiLang) -> &'static str {
    pick(lang, "System prompt", "系统提示")
}

pub fn ai_system_prompt_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Send Tinycast’s preamble and your extra instructions.",
        "发送 Tinycast 前导说明和你的附加指令。",
    )
}

pub fn ai_opens_to_title(lang: UiLang) -> &'static str {
    pick(lang, "Opens to", "打开时显示")
}

pub fn ai_opens_to_value(new_conversation: bool, lang: UiLang) -> &'static str {
    if new_conversation {
        pick(lang, "A New Conversation", "新对话")
    } else {
        pick(lang, "Recent Conversation", "最近对话")
    }
}

pub fn ai_keep_conversations(lang: UiLang) -> &'static str {
    pick(lang, "Keep conversations", "保留对话")
}

pub fn ai_retention_value(days: i64, lang: UiLang) -> &'static str {
    match (days, lang) {
        (7, UiLang::En) => "7 Days",
        (7, UiLang::ZhHans) => "7 天",
        (90, UiLang::En) => "3 Months",
        (90, UiLang::ZhHans) => "3 个月",
        (d, UiLang::En) if d < 0 => "Forever",
        (d, UiLang::ZhHans) if d < 0 => "永久",
        (_, UiLang::En) => "30 Days",
        (_, UiLang::ZhHans) => "30 天",
    }
}

pub fn ai_default_model(lang: UiLang) -> &'static str {
    pick(lang, "Default model", "默认模型")
}

pub fn ai_none(lang: UiLang) -> &'static str {
    pick(lang, "None", "无")
}

pub fn ai_remove_title(lang: UiLang) -> &'static str {
    pick(lang, "Remove API connection?", "移除 API 连接？")
}

pub fn ai_remove_message(name: &str, lang: UiLang) -> String {
    match lang {
        UiLang::En => format!("Remove {name} from this PC? The saved key is deleted."),
        UiLang::ZhHans => format!("从这台电脑移除 {name}？已保存的密钥会一并删除。"),
    }
}

pub fn ai_remove_action(lang: UiLang) -> &'static str {
    pick(lang, "Remove", "移除")
}

pub fn ai_save_failed(lang: UiLang) -> &'static str {
    pick(lang, "Couldn’t save this connection.", "无法保存此连接。")
}

pub fn ai_chatgpt_subscription(lang: UiLang) -> &'static str {
    pick(lang, "ChatGPT subscription", "ChatGPT 订阅")
}

pub fn ai_add_connection(lang: UiLang) -> &'static str {
    pick(lang, "Add API connection", "添加 API 连接")
}

pub fn ai_add_connection_sub(lang: UiLang) -> &'static str {
    pick(
        lang,
        "OpenAI, Anthropic, Gemini, OpenRouter, or compatible.",
        "OpenAI、Anthropic、Gemini、OpenRouter 或兼容接口。",
    )
}

pub fn ai_saved_on_pc(lang: UiLang) -> &'static str {
    pick(lang, "Saved on this PC", "保存在这台电脑")
}

pub fn ai_not_set(lang: UiLang) -> &'static str {
    pick(lang, "Not set", "未设置")
}

pub fn ai_provider(lang: UiLang) -> &'static str {
    pick(lang, "Provider", "提供商")
}

pub fn ai_base_url(lang: UiLang) -> &'static str {
    pick(lang, "Base URL", "接口地址")
}

pub fn ai_model_id(lang: UiLang) -> &'static str {
    pick(lang, "Model id", "模型 ID")
}

pub fn ai_api_key(lang: UiLang) -> &'static str {
    pick(lang, "API key", "API 密钥")
}

pub fn ai_connect(lang: UiLang) -> &'static str {
    pick(lang, "Connect", "连接")
}

pub fn ai_disconnect_chatgpt(lang: UiLang) -> &'static str {
    pick(lang, "Disconnect ChatGPT", "断开 ChatGPT")
}

pub fn ai_try_again(lang: UiLang) -> &'static str {
    pick(lang, "Try Again", "重试")
}

pub fn ai_install_codex(lang: UiLang) -> &'static str {
    pick(lang, "Install Codex CLI…", "安装 Codex CLI…")
}

pub fn ai_not_available(lang: UiLang) -> &'static str {
    pick(lang, "Not available yet", "暂不可用")
}

pub fn qa_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Enable Quick Actions", "启用快捷操作")
}

pub fn qa_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Act on the selected text in the previous app. Off by default.",
        "对上一应用中的选中文本执行操作。默认关闭。",
    )
}

pub fn qa_translate_into(lang: UiLang) -> &'static str {
    pick(lang, "Translate into", "翻译为")
}

pub fn qa_language_name(raw: &str, lang: UiLang) -> &'static str {
    match (raw, lang) {
        ("Spanish", UiLang::ZhHans) => "西班牙语",
        ("French", UiLang::ZhHans) => "法语",
        ("German", UiLang::ZhHans) => "德语",
        ("English", UiLang::ZhHans) => "英语",
        ("Spanish", _) => "Spanish",
        ("French", _) => "French",
        ("German", _) => "German",
        (_, UiLang::ZhHans) => "英语",
        _ => "English",
    }
}

pub fn clipboard_history(lang: UiLang) -> &'static str {
    pick(lang, "History", "历史")
}

pub fn clipboard_keep_for(lang: UiLang) -> &'static str {
    pick(lang, "Keep history for", "历史保留时间")
}

pub fn clipboard_retention(days: i64, lang: UiLang) -> &'static str {
    match (days, lang) {
        (1, UiLang::En) => "1 Day",
        (1, UiLang::ZhHans) => "1 天",
        (7, UiLang::En) => "1 Week",
        (7, UiLang::ZhHans) => "1 周",
        (30, UiLang::En) => "1 Month",
        (30, UiLang::ZhHans) => "1 个月",
        (90, UiLang::En) => "3 Months",
        (90, UiLang::ZhHans) => "3 个月",
        (180, UiLang::En) => "6 Months",
        (180, UiLang::ZhHans) => "6 个月",
        (365, UiLang::En) => "1 Year",
        (365, UiLang::ZhHans) => "1 年",
        (_, UiLang::En) => "Forever",
        (_, UiLang::ZhHans) => "永久",
    }
}

pub fn clipboard_disabled_apps(lang: UiLang) -> &'static str {
    pick(lang, "Disabled Applications", "已禁用的应用")
}

pub fn clipboard_disabled_caption(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Copies from these apps are not recorded.",
        "不会记录这些应用中的复制内容。",
    )
}

pub fn clipboard_add_application(lang: UiLang) -> &'static str {
    pick(lang, "Add Application…", "添加应用…")
}

pub fn clipboard_add(lang: UiLang) -> &'static str {
    pick(lang, "Add", "添加")
}

pub fn clipboard_clear_section(lang: UiLang) -> &'static str {
    pick(lang, "Clear", "清除")
}

pub fn clipboard_clear_history(lang: UiLang) -> &'static str {
    pick(lang, "Clear history", "清除历史")
}

pub fn clipboard_clear_ellipsis(lang: UiLang) -> &'static str {
    pick(lang, "Clear…", "清除…")
}

pub fn clipboard_confirm_title(lang: UiLang) -> &'static str {
    pick(lang, "Clear clipboard history?", "清除剪贴板历史？")
}

pub fn clipboard_confirm_message(lang: UiLang) -> &'static str {
    pick(lang, "This can't be undone.", "此操作无法撤销。")
}

pub fn clipboard_confirm_action(lang: UiLang) -> &'static str {
    pick(lang, "Clear History", "清除历史")
}

pub fn notes_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Enable Notes", "启用笔记")
}

pub fn notes_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "A local Markdown editor. Off by default; enabling does not create notes until you ask.",
        "本地 Markdown 编辑器。默认关闭；开启后不会自动创建笔记。",
    )
}

pub fn snippets_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Snippets", "片段")
}

pub fn snippets_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Keyword expansion requires listening to keystrokes. Keystrokes stay on this PC.",
        "关键词展开需要监听按键。按键记录留在这台电脑上。",
    )
}

pub fn snippets_show_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Hide the Snippets section without turning keyword expansion off.",
        "隐藏片段分区，但不关闭关键词展开。",
    )
}

pub fn snippets_confirm_title(lang: UiLang) -> &'static str {
    pick(lang, "Enable snippets?", "启用片段？")
}

pub fn snippets_confirm_message(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Keyword expansion requires the Accessibility permission. Keystrokes stay on this PC.",
        "关键词展开需要辅助功能权限。按键记录留在这台电脑上。",
    )
}

pub fn snippets_confirm_action(lang: UiLang) -> &'static str {
    pick(lang, "Continue", "继续")
}

pub fn file_search_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Enable File Search", "启用文件搜索")
}

pub fn file_search_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Find files and folders through Windows Search, only on demand. Off by default.",
        "按需通过 Windows 搜索查找文件和文件夹。默认关闭。",
    )
}

pub fn file_search_folders(lang: UiLang) -> &'static str {
    pick(lang, "Folders", "文件夹")
}

pub fn file_search_ignore(lang: UiLang) -> &'static str {
    pick(lang, "Ignore patterns", "忽略规则")
}

pub fn file_search_add_folder(lang: UiLang) -> &'static str {
    pick(lang, "Add folder…", "添加文件夹…")
}

pub fn file_search_add_ignore(lang: UiLang) -> &'static str {
    pick(lang, "Add ignore pattern…", "添加忽略规则…")
}

pub fn window_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Window Management", "窗口管理")
}

pub fn window_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Move and resize the frontmost window. Off by default.",
        "移动和调整前台窗口大小。默认关闭。",
    )
}

pub fn window_show_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Hide the Window Management section without disabling shortcuts.",
        "隐藏窗口管理分区，但不关闭快捷键。",
    )
}

pub fn window_cycle_title(lang: UiLang) -> &'static str {
    pick(lang, "Cycle halves on repeat", "重复时循环对半")
}

pub fn window_cycle_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Repeated Left/Right/Top/Bottom Half cycles ½ → ⅓ → ⅔.",
        "重复左/右/上/下半屏时按 ½ → ⅓ → ⅔ 循环。",
    )
}

pub fn window_gap_title(lang: UiLang) -> &'static str {
    pick(lang, "Gap", "间距")
}

pub fn window_gap_subtitle(gap: i32, lang: UiLang) -> String {
    match lang {
        UiLang::En => format!("{gap} pt between tiles and screen edges."),
        UiLang::ZhHans => format!("贴片与屏幕边缘间距 {gap} pt。"),
    }
}

pub fn extensions_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Enable extensions", "启用扩展")
}

pub fn extensions_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Run Raycast extensions natively. A running command holds a JavaScript engine in memory until you leave it.",
        "原生运行 Raycast 扩展。正在运行的命令会在内存中保留 JavaScript 引擎，直到你离开。",
    )
}

pub fn extensions_show_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "List every extension's commands in launcher search.",
        "在启动器搜索中列出每个扩展的命令。",
    )
}

pub fn extensions_runtime_notice(lang: UiLang) -> &'static str {
    pick(
        lang,
        "The extension runtime is not in this version.",
        "此版本不包含扩展运行时。",
    )
}

pub fn quicklinks_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Quicklinks", "快捷链接")
}

pub fn quicklinks_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Open URLs, files, and searches from the launcher. Off by default.",
        "从启动器打开网址、文件和搜索。默认关闭。",
    )
}

pub fn quicklinks_show_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Hide the Quicklinks section without turning shortcuts off.",
        "隐藏快捷链接分区，但不关闭快捷键。",
    )
}

pub fn quicklinks_create(lang: UiLang) -> &'static str {
    pick(lang, "Create", "新建")
}

pub fn quicklinks_import(lang: UiLang) -> &'static str {
    pick(lang, "Import", "导入")
}

pub fn quicklinks_export(lang: UiLang) -> &'static str {
    pick(lang, "Export", "导出")
}

pub fn custom_commands_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Custom Commands", "自定义命令")
}

pub fn custom_commands_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Run your own commands from the launcher. Off by default.",
        "从启动器运行你自己的命令。默认关闭。",
    )
}

pub fn custom_commands_show_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Hide the Custom Commands section without turning shortcuts off.",
        "隐藏自定义命令分区，但不关闭快捷键。",
    )
}

pub fn custom_commands_new(lang: UiLang) -> &'static str {
    pick(lang, "New command", "新建命令")
}

pub fn emoji_skin_tone_title(lang: UiLang) -> &'static str {
    pick(lang, "Skin tone", "肤色")
}

pub fn emoji_skin_tone_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Applied to people emoji in Search Emoji.",
        "应用于表情搜索中的人物表情。",
    )
}

pub fn emoji_skin_tone_label(raw: &str, lang: UiLang) -> &'static str {
    match (raw, lang) {
        ("light", UiLang::ZhHans) => "浅色",
        ("medium-light", UiLang::ZhHans) => "中浅",
        ("medium", UiLang::ZhHans) => "中等",
        ("medium-dark", UiLang::ZhHans) => "中深",
        ("dark", UiLang::ZhHans) => "深色",
        ("none", UiLang::ZhHans) | ("", UiLang::ZhHans) => "默认",
        ("light", _) => "Light",
        ("medium-light", _) => "Medium-Light",
        ("medium", _) => "Medium",
        ("medium-dark", _) => "Medium-Dark",
        ("dark", _) => "Dark",
        _ => pick(lang, "Default", "默认"),
    }
}

pub fn calendar_enable_title(lang: UiLang) -> &'static str {
    pick(lang, "Enable Calendar", "启用日历")
}

pub fn calendar_enable_subtitle(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Read this PC’s calendar. Off by default.",
        "读取这台电脑的日历。默认关闭。",
    )
}

pub fn calendar_auto_join(lang: UiLang) -> &'static str {
    pick(lang, "Auto-join meetings", "自动加入会议")
}

pub fn calendar_auto_join_sub(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Open the join link when a meeting starts. Once per meeting per launch.",
        "会议开始时打开加入链接。每次启动每个会议只触发一次。",
    )
}

pub fn calendar_camera(lang: UiLang) -> &'static str {
    pick(lang, "Camera preview", "摄像头预览")
}

pub fn calendar_camera_sub(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Optional preview before joining. Deny is not fatal.",
        "加入前可选预览。拒绝不会导致失败。",
    )
}

pub fn calendar_join_window(lang: UiLang) -> &'static str {
    pick(lang, "Join window", "加入窗口")
}

pub fn calendar_join_minutes(minutes: i64, lang: UiLang) -> &'static str {
    match (minutes, lang) {
        (1, UiLang::En) => "1 minute",
        (1, UiLang::ZhHans) => "1 分钟",
        (2, UiLang::En) => "2 minutes",
        (2, UiLang::ZhHans) => "2 分钟",
        (10, UiLang::En) => "10 minutes",
        (10, UiLang::ZhHans) => "10 分钟",
        (15, UiLang::En) => "15 minutes",
        (15, UiLang::ZhHans) => "15 分钟",
        (_, UiLang::En) => "5 minutes",
        (_, UiLang::ZhHans) => "5 分钟",
    }
}

pub fn pane_section_title(tab: SettingsTab, lang: UiLang) -> &'static str {
    settings_tab_title(tab, lang)
}

pub fn cancel_label(lang: UiLang) -> &'static str {
    chrome(Chrome::Cancel, lang)
}

pub fn reset_ranking_action(lang: UiLang) -> &'static str {
    chrome(Chrome::ResetRanking, lang)
}
