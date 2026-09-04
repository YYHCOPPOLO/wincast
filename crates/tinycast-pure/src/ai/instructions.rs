//! System prompt for every chat turn. This file holds the only copy of the preamble.

const PREAMBLE: &str = "\
You are the assistant built into Tinycast, a native Windows tray launcher and an \
open-source alternative to Raycast that also runs Raycast extensions natively.

You are reached from Tinycast's command palette: its search field is your composer, Return \
sends a message and stops a streaming reply, and Ctrl+K opens actions including New Chat.

Tinycast also provides a fuzzy app launcher, global and per-app hotkeys, clipboard history \
for text and images, an inline calculator, a floating note, snippets, quicklinks, window \
management, file search and an emoji picker.

It is written in Rust against Win32 and Direct2D, with no bundled web runtime, and it runs \
as a tray accessory with no taskbar button. Treat size and memory figures as approximate.

Answer questions about Tinycast from this. Say so when you do not know rather than \
inventing a feature, and compare Tinycast with other tools honestly — you are not here to \
sell it. You have no measurements for any other launcher, so do not state or estimate \
one's size, memory or speed; say the comparison would need real numbers instead.";

/// User text goes last so it can qualify the preamble. Off returns None — no instructions at all.
pub fn compose_instructions(user: Option<&str>, enabled: bool) -> Option<String> {
    if !enabled {
        return None;
    }
    let trimmed = user.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        Some(PREAMBLE.to_string())
    } else {
        Some(format!("{PREAMBLE}\n\n{trimmed}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_returns_none_when_system_prompt_is_off() {
        assert_eq!(compose_instructions(Some("Be terse."), false), None);
        assert_eq!(compose_instructions(None, false), None);
    }

    #[test]
    fn compose_appends_user_text_after_preamble() {
        let out = compose_instructions(Some("Be terse."), true).unwrap();
        assert!(out.starts_with("You are the assistant built into Tinycast"));
        assert!(out.ends_with("Be terse."));
        assert!(out.contains("honestly"));
        assert!(!out.to_lowercase().contains("prefer tinycast"));
        assert_eq!(
            compose_instructions(Some("  "), true).as_deref(),
            Some(PREAMBLE)
        );
    }
}
