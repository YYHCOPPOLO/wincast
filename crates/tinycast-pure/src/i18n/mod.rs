mod catalog;
mod copy;
mod panes;
mod surfaces;

pub use catalog::*;
pub use copy::*;
pub use panes::*;
pub use surfaces::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiLang {
    #[default]
    ZhHans,
    En,
}

impl UiLang {
    pub fn parse(raw: &str) -> Self {
        match raw.trim() {
            "en" | "en-US" | "en-us" => UiLang::En,
            _ => UiLang::ZhHans,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            UiLang::ZhHans => "zh-Hans",
            UiLang::En => "en",
        }
    }

    pub fn dwrite_locale(self) -> &'static str {
        match self {
            UiLang::ZhHans => "zh-CN",
            UiLang::En => "en-US",
        }
    }

    pub fn other(self) -> Self {
        match self {
            UiLang::ZhHans => UiLang::En,
            UiLang::En => UiLang::ZhHans,
        }
    }

    pub fn cycle(self) -> Self {
        self.other()
    }
}
