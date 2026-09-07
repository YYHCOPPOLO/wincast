use crate::command_id::CommandID;
use crate::i18n::UiLang;

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

#[cfg(test)]
mod tests {
    use crate::command_id::CommandID;
    use crate::i18n::{command_title, UiLang};

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
        assert_eq!(command_title(CommandID::Quit, UiLang::ZhHans), "退出 Tinycast");
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
}
