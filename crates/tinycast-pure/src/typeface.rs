//! Which typeface draws which text. Windows-native stand-ins for the macOS system font:
//! SF Pro cannot be redistributed off Apple platforms, so Latin text uses Segoe UI
//! Variable (Windows 11) with the optical size picked the way SF picks Text vs Display,
//! and CJK text falls back to Microsoft YaHei UI.

/// Segoe UI Variable's optical-size families, smallest first.
pub const LATIN_SMALL: &str = "Segoe UI Variable Small";
pub const LATIN_TEXT: &str = "Segoe UI Variable Text";
pub const LATIN_DISPLAY: &str = "Segoe UI Variable Display";
/// Windows 10 / missing-font fallback for Latin.
pub const LATIN_FALLBACK: &str = "Segoe UI";
/// CJK families, preferred first.
pub const CJK: [&str; 2] = ["Microsoft YaHei UI", "Microsoft YaHei"];

/// The native Windows icon fonts. Segoe Fluent Icons ships with Windows 11;
/// Windows 10 carries the older Segoe MDL2 Assets, which shares the code
/// points the glyph map uses. The file name is the reliable probe: font
/// enumeration would have to come from DirectWrite for no extra benefit.
pub const ICON_FLUENT: &str = "Segoe Fluent Icons";
pub const ICON_MDL2: &str = "Segoe MDL2 Assets";

pub fn icon_family() -> &'static str {
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    let fluent = std::path::Path::new(&windir)
        .join("Fonts")
        .join("SegoeIcons.ttf");
    if fluent.is_file() {
        ICON_FLUENT
    } else {
        ICON_MDL2
    }
}

/// The Segoe UI Variable optical size for a DIP size. Fluent's type ramp switches to
/// Display at 20 and to Small below 12, which is also where SF Pro swaps optical sizes.
pub fn latin_family_for_size(size_dip: f32) -> &'static str {
    if size_dip >= 20.0 {
        LATIN_DISPLAY
    } else if size_dip < 12.0 {
        LATIN_SMALL
    } else {
        LATIN_TEXT
    }
}

/// Families to try for a size, in order, until one is installed.
pub fn latin_chain(size_dip: f32) -> [&'static str; 3] {
    [latin_family_for_size(size_dip), LATIN_TEXT, LATIN_FALLBACK]
}

/// Code point ranges handed to the CJK fallback before the system fallback runs:
/// CJK radicals through Yi, Hangul syllables, compatibility ideographs and forms,
/// half/full-width forms, and the supplementary ideograph planes.
pub const CJK_RANGES: [(u32, u32); 7] = [
    (0x2E80, 0x9FFF),
    (0xA000, 0xA4CF),
    (0xAC00, 0xD7AF),
    (0xF900, 0xFAFF),
    (0xFE30, 0xFE4F),
    (0xFF00, 0xFFEF),
    (0x20000, 0x3FFFF),
];

pub fn is_cjk(c: char) -> bool {
    let u = c as u32;
    CJK_RANGES.iter().any(|&(a, b)| u >= a && u <= b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optical_sizes_follow_the_ramp() {
        assert_eq!(latin_family_for_size(20.0), LATIN_DISPLAY);
        assert_eq!(latin_family_for_size(22.0), LATIN_DISPLAY);
        assert_eq!(latin_family_for_size(13.0), LATIN_TEXT);
        assert_eq!(latin_family_for_size(12.0), LATIN_TEXT);
        assert_eq!(latin_family_for_size(11.0), LATIN_SMALL);
        assert_eq!(latin_family_for_size(10.0), LATIN_SMALL);
    }

    #[test]
    fn chain_ends_in_segoe_ui() {
        assert_eq!(latin_chain(13.0)[2], "Segoe UI");
        assert_eq!(latin_chain(10.0)[0], LATIN_SMALL);
    }

    #[test]
    fn cjk_ranges_cover_han_kana_hangul_fullwidth() {
        for c in ['中', '文', 'あ', 'カ', '한', '，', '。', 'Ａ'] {
            assert!(is_cjk(c), "{c}");
        }
        for c in ['a', 'Z', '1', 'é', '→', '⌘'] {
            assert!(!is_cjk(c), "{c}");
        }
    }

    #[test]
    fn cjk_prefers_yahei_ui() {
        assert_eq!(CJK[0], "Microsoft YaHei UI");
    }
}
