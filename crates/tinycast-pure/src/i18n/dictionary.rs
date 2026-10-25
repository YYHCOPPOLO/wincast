//! Labels for the ECDICT lookup panel: the search hint, the Tab captions, and
//! every section header the formatter prints.

use crate::dictionary::ExchangeKind;
use crate::i18n::UiLang;

fn pick<'a>(lang: UiLang, en: &'a str, zh: &'a str) -> &'a str {
    match lang {
        UiLang::En => en,
        UiLang::ZhHans => zh,
    }
}

/// Header field hint while the palette is in dictionary mode.
pub fn dictionary_placeholder(lang: UiLang) -> &'static str {
    pick(lang, "Type a word or phrase…", "输入单词或词语…")
}

/// Body hint for an empty query: the panel has nothing to look up yet.
pub fn dictionary_hint(lang: UiLang) -> &'static str {
    pick(
        lang,
        "Type a word or phrase to look it up.",
        "输入单词或词语以查询。",
    )
}

/// Tab caption on the launcher while a query could open the panel.
pub fn dictionary_tab_define(lang: UiLang) -> &'static str {
    pick(lang, "Tab to define", "Tab 查词")
}

/// Tab caption inside the panel; AI is the ring's next stop when enabled.
pub fn dictionary_tab_ai(lang: UiLang) -> &'static str {
    pick(lang, "Tab AI", "Tab AI")
}

/// Tab caption inside the panel when AI is off, so Tab rings to Clipboard.
pub fn dictionary_tab_clipboard(lang: UiLang) -> &'static str {
    pick(lang, "Tab clipboard", "Tab 剪贴板")
}

pub fn dictionary_section_chinese(lang: UiLang) -> &'static str {
    pick(lang, "Chinese", "中文释义")
}

pub fn dictionary_section_english(lang: UiLang) -> &'static str {
    pick(lang, "English", "英文释义")
}

pub fn dictionary_section_forms(lang: UiLang) -> &'static str {
    pick(lang, "Forms", "词形变化")
}

pub fn dictionary_section_pos(lang: UiLang) -> &'static str {
    pick(lang, "Parts of speech", "词性分布")
}

pub fn dictionary_section_frequency(lang: UiLang) -> &'static str {
    pick(lang, "Frequency", "词频")
}

pub fn dictionary_section_more(lang: UiLang) -> &'static str {
    pick(lang, "More", "更多信息")
}

pub fn dictionary_exchange_label(kind: ExchangeKind, lang: UiLang) -> &'static str {
    match (kind, lang) {
        (ExchangeKind::Lemma, UiLang::En) => "Lemma",
        (ExchangeKind::Lemma, UiLang::ZhHans) => "原型",
        (ExchangeKind::Past, UiLang::En) => "Past",
        (ExchangeKind::Past, UiLang::ZhHans) => "过去式",
        (ExchangeKind::PastParticiple, UiLang::En) => "Past participle",
        (ExchangeKind::PastParticiple, UiLang::ZhHans) => "过去分词",
        (ExchangeKind::PresentParticiple, UiLang::En) => "Present participle",
        (ExchangeKind::PresentParticiple, UiLang::ZhHans) => "现在分词",
        (ExchangeKind::ThirdPerson, UiLang::En) => "Third person",
        (ExchangeKind::ThirdPerson, UiLang::ZhHans) => "第三人称单数",
        (ExchangeKind::Comparative, UiLang::En) => "Comparative",
        (ExchangeKind::Comparative, UiLang::ZhHans) => "比较级",
        (ExchangeKind::Superlative, UiLang::En) => "Superlative",
        (ExchangeKind::Superlative, UiLang::ZhHans) => "最高级",
        (ExchangeKind::Plural, UiLang::En) => "Plural",
        (ExchangeKind::Plural, UiLang::ZhHans) => "复数",
        (ExchangeKind::DerivedForms, UiLang::En) => "Other forms",
        (ExchangeKind::DerivedForms, UiLang::ZhHans) => "其他形式",
    }
}

pub fn dictionary_collins_label(lang: UiLang) -> &'static str {
    pick(lang, "Collins", "柯林斯")
}

pub fn dictionary_oxford_label(lang: UiLang) -> &'static str {
    pick(lang, "Oxford 3000", "牛津 3000")
}

pub fn dictionary_bnc_label(lang: UiLang) -> &'static str {
    pick(lang, "BNC", "BNC")
}

pub fn dictionary_coca_label(lang: UiLang) -> &'static str {
    pick(lang, "COCA", "COCA")
}

/// Exam/level tags ECDICT stores in `tag`. An unknown tag is printed raw.
pub fn dictionary_tag_label(tag: &str, lang: UiLang) -> Option<&'static str> {
    match (tag, lang) {
        ("zk", UiLang::En) => Some("Zhongkao"),
        ("zk", UiLang::ZhHans) => Some("中考"),
        ("gk", UiLang::En) => Some("Gaokao"),
        ("gk", UiLang::ZhHans) => Some("高考"),
        ("cet4", UiLang::En) => Some("CET-4"),
        ("cet4", UiLang::ZhHans) => Some("四级"),
        ("cet6", UiLang::En) => Some("CET-6"),
        ("cet6", UiLang::ZhHans) => Some("六级"),
        ("ky", UiLang::En) => Some("Kaoyan"),
        ("ky", UiLang::ZhHans) => Some("考研"),
        ("ielts", UiLang::En) => Some("IELTS"),
        ("ielts", UiLang::ZhHans) => Some("雅思"),
        ("toefl", UiLang::En) => Some("TOEFL"),
        ("toefl", UiLang::ZhHans) => Some("托福"),
        ("gre", UiLang::En) => Some("GRE"),
        ("gre", UiLang::ZhHans) => Some("GRE"),
        _ => None,
    }
}

pub fn dictionary_no_entry(lang: UiLang) -> &'static str {
    pick(lang, "No entry found", "未找到词条")
}

pub fn dictionary_no_entry_detail(query: &str, lang: UiLang) -> String {
    match lang {
        UiLang::En => format!("“{query}” is not in the dictionary."),
        UiLang::ZhHans => format!("词典中没有「{query}」。"),
    }
}

pub fn dictionary_closest(lang: UiLang) -> &'static str {
    pick(lang, "Closest", "相近词条")
}

pub fn dictionary_copy_definition(lang: UiLang) -> &'static str {
    pick(lang, "Copy Definition", "复制释义")
}

pub fn dictionary_definition_copied(lang: UiLang) -> &'static str {
    pick(lang, "Definition copied", "已复制释义")
}
