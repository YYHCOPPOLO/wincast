use std::path::{Path, PathBuf};

use super::model::{default_name, SnippetSourceRevision, StoredSnippet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodecError {
    InvalidFrontmatter {
        path: PathBuf,
        line: usize,
        reason: String,
    },
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::InvalidFrontmatter { path, line, reason } => {
                write!(f, "{}:{line}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for CodecError {}

pub fn parse_markdown(path: &Path, bytes: &str) -> Result<StoredSnippet, CodecError> {
    let lines = source_lines(bytes);
    if lines.first().map(|l| l.text.as_str()) != Some("---") {
        return Ok(stored(
            path,
            default_name(path),
            None,
            true,
            false,
            bytes.to_string(),
            bytes,
        ));
    }

    let closing = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, line)| line.text == "---")
        .map(|(i, _)| i)
        .ok_or_else(|| parse_error(path, 1, "Missing closing frontmatter delimiter"))?;

    let mut name: Option<String> = None;
    let mut keyword: Option<String> = None;
    let mut enabled = true;
    let mut show_confirmation = false;
    let mut seen = std::collections::HashSet::new();

    for (idx, line) in lines.iter().enumerate().take(closing).skip(1) {
        let line_number = idx + 1;
        if line.text.trim().is_empty() {
            continue;
        }
        let Some(sep) = line.text.find(':') else {
            return Err(parse_error(
                path,
                line_number,
                "Expected a key and value separated by ':'",
            ));
        };
        let raw_key = line.text[..sep].trim();
        let raw_value = line.text[sep + 1..].trim();
        let Some(key) = canonical_key(raw_key) else {
            return Err(parse_error(
                path,
                line_number,
                &format!("Unsupported frontmatter key '{raw_key}'"),
            ));
        };
        if !seen.insert(key) {
            return Err(parse_error(
                path,
                line_number,
                &format!("Duplicate frontmatter key '{key}'"),
            ));
        }
        match key {
            "name" => {
                let decoded = decode_scalar(raw_value, path, line_number)?;
                name = if decoded.trim().is_empty() {
                    None
                } else {
                    Some(decoded)
                };
            }
            "keyword" => keyword = Some(decode_scalar(raw_value, path, line_number)?),
            "enabled" => enabled = decode_boolean(raw_value, path, line_number)?,
            "show_confirmation" => {
                show_confirmation = decode_boolean(raw_value, path, line_number)?;
            }
            _ => unreachable!(),
        }
    }

    let body = bytes[lines[closing].end..].to_string();
    Ok(stored(
        path,
        name.unwrap_or_else(|| default_name(path)),
        keyword,
        enabled,
        show_confirmation,
        body,
        bytes,
    ))
}

pub fn serialize(s: &StoredSnippet) -> String {
    let mut lines = vec![
        "---".to_string(),
        format!("name: {}", encode_scalar(&s.name)),
    ];
    if let Some(keyword) = &s.keyword {
        lines.push(format!("keyword: {}", encode_scalar(keyword)));
    }
    lines.push(format!("enabled: {}", s.enabled));
    lines.push(format!("show_confirmation: {}", s.show_confirmation));
    lines.push("---".into());
    format!("{}\n{}", lines.join("\n"), s.body)
}

fn stored(
    path: &Path,
    name: String,
    keyword: Option<String>,
    enabled: bool,
    show_confirmation: bool,
    body: String,
    source: &str,
) -> StoredSnippet {
    StoredSnippet {
        path: path.to_path_buf(),
        name,
        keyword,
        enabled,
        show_confirmation,
        body,
        source_revision: SnippetSourceRevision::new(source),
    }
}

struct SourceLine {
    text: String,
    end: usize,
}

fn source_lines(content: &str) -> Vec<SourceLine> {
    let bytes = content.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' || bytes[i] == b'\r' {
            let mut end = i + 1;
            if bytes[i] == b'\r' && end < bytes.len() && bytes[end] == b'\n' {
                end += 1;
            }
            lines.push(SourceLine {
                text: content[start..i].to_string(),
                end,
            });
            start = end;
            i = end;
        } else {
            i += 1;
        }
    }
    if start < content.len() || lines.is_empty() {
        lines.push(SourceLine {
            text: content[start..].to_string(),
            end: content.len(),
        });
    }
    lines
}

fn canonical_key(raw: &str) -> Option<&'static str> {
    match raw.to_ascii_lowercase().as_str() {
        "name" => Some("name"),
        "keyword" => Some("keyword"),
        "enabled" => Some("enabled"),
        "show_confirmation" => Some("show_confirmation"),
        _ => None,
    }
}

fn decode_scalar(value: &str, path: &Path, line: usize) -> Result<String, CodecError> {
    let mut chars = value.chars().peekable();
    if chars.next() != Some('"') {
        return Err(parse_error(
            path,
            line,
            "String values must use double quotes",
        ));
    }
    let mut decoded = String::new();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            if chars.any(|c| c != ' ' && c != '\t') {
                return Err(parse_error(
                    path,
                    line,
                    "Unexpected text after quoted value",
                ));
            }
            return Ok(decoded);
        }
        if ch == '\\' {
            let Some(esc) = chars.next() else {
                return Err(parse_error(path, line, "Unterminated escape sequence"));
            };
            match esc {
                '\\' => decoded.push('\\'),
                '"' => decoded.push('"'),
                'n' => decoded.push('\n'),
                'r' => decoded.push('\r'),
                't' => decoded.push('\t'),
                _ => return Err(parse_error(path, line, "Unsupported escape sequence")),
            }
            continue;
        }
        if ch == '\n' || ch == '\r' || ch == '\t' {
            return Err(parse_error(
                path,
                line,
                "Control characters must be escaped",
            ));
        }
        decoded.push(ch);
    }
    Err(parse_error(path, line, "Unterminated quoted value"))
}

fn decode_boolean(value: &str, path: &Path, line: usize) -> Result<bool, CodecError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(parse_error(
            path,
            line,
            "Boolean values must be exactly 'true' or 'false'",
        )),
    }
}

fn encode_scalar(value: &str) -> String {
    let mut encoded = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => encoded.push_str("\\\\"),
            '"' => encoded.push_str("\\\""),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            c => encoded.push(c),
        }
    }
    encoded.push('"');
    encoded
}

fn parse_error(path: &Path, line: usize, reason: &str) -> CodecError {
    CodecError::InvalidFrontmatter {
        path: path.to_path_buf(),
        line,
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse(path: &str, raw: &str) -> StoredSnippet {
        parse_markdown(Path::new(path), raw).unwrap()
    }

    fn expect_err(raw: &str) {
        assert!(
            parse_markdown(Path::new("/tmp/codec.md"), raw).is_err(),
            "{raw}"
        );
    }

    #[test]
    fn round_trip_canonical_frontmatter() {
        let raw = "---\nname: \"Meeting Notes\"\nkeyword: \"!notes\"\nenabled: true\nshow_confirmation: false\n---\n\nHello {clipboard}\n";
        let s = parse_markdown(Path::new("Meeting Notes.md"), raw).unwrap();
        assert_eq!(s.name, "Meeting Notes");
        assert_eq!(s.keyword.as_deref(), Some("!notes"));
        assert!(serialize(&s).starts_with("---\nname:"));
    }

    #[test]
    fn round_trips_escaped_quoted_scalars() {
        let snippet = StoredSnippet {
            path: PathBuf::from("/tmp/codec.md"),
            name: "Quote \" slash \\ line\nreturn\rtab\t雪".into(),
            keyword: Some("!\"\\\n\t".into()),
            enabled: false,
            show_confirmation: true,
            body: "\nFirst body line\n\nLast body line\r\n".into(),
            source_revision: SnippetSourceRevision::new(""),
        };
        let serialized = serialize(&snippet);
        let parsed = parse_markdown(Path::new("/tmp/codec.md"), &serialized).unwrap();
        assert_eq!(parsed.name, snippet.name);
        assert_eq!(parsed.keyword, snippet.keyword);
        assert_eq!(parsed.enabled, snippet.enabled);
        assert_eq!(parsed.show_confirmation, snippet.show_confirmation);
        assert_eq!(parsed.body, snippet.body);
        assert!(serialized.starts_with(
            "---\nname: \"Quote \\\" slash \\\\ line\\nreturn\\rtab\\t雪\"\nkeyword: \"!\\\"\\\\\\n\\t\"\nenabled: false\nshow_confirmation: true\n---\n"
        ));
    }

    #[test]
    fn quoted_scalar_encoding_prevents_line_injection() {
        let snippet = StoredSnippet {
            path: PathBuf::from("/tmp/codec.md"),
            name: "Safe\"\nenabled: false".into(),
            keyword: None,
            enabled: true,
            show_confirmation: false,
            body: "Body".into(),
            source_revision: SnippetSourceRevision::new(""),
        };
        let source = serialize(&snippet);
        assert!(!source.contains("\nenabled: false\nenabled:"));
        let parsed = parse_markdown(Path::new("/tmp/codec.md"), &source).unwrap();
        assert_eq!(parsed.name, snippet.name);
    }

    #[test]
    fn crlf_frontmatter_and_body_boundaries() {
        let crlf = "---\r\nname: \"CRLF\"\r\nshow_confirmation: true\r\n---\r\n\r\nBody\r\n";
        let parsed = parse("/tmp/codec.md", crlf);
        assert!(parsed.show_confirmation);
        assert_eq!(parsed.body, "\r\nBody\r\n");
        let missing = parse("/tmp/codec.md", "---\nname: \"No HUD\"\n---\nBody");
        assert!(!missing.show_confirmation);
        expect_err("---\nshow_confirmation: TRUE\n---\n");
    }

    #[test]
    fn body_delimiters_and_filename_fallback() {
        let parsed = parse(
            "/tmp/codec.md",
            "---\nname: \"Delimiter Body\"\nenabled: true\n---\nFirst\n---\nLast\n",
        );
        assert_eq!(parsed.body, "First\n---\nLast\n");
        let body_only = "--- not frontmatter\n\nBody";
        let parsed = parse("/tmp/body-only-name.md", body_only);
        assert_eq!(parsed.body, body_only);
        assert_eq!(parsed.name, "Body Only Name");
        let blank = parse("/tmp/blank-name-file.md", "---\nname: \" \\t \"\n---\nBody");
        let empty = parse("/tmp/blank-name-file.md", "---\nname: \"\"\n---\nBody");
        assert_eq!(blank.name, "Blank Name File");
        assert_eq!(empty.name, "Blank Name File");
    }

    #[test]
    fn rejects_malformed_frontmatter() {
        expect_err("---\nname: \"Broken\"\n");
        expect_err("---\nname: \"Broken\"\n--- \n");
        expect_err("---\nname: Broken\n---\n");
        expect_err("---\nname: \"Bad\\q\"\n---\n");
        expect_err("---\nenabled: FALSE\n---\n");
        expect_err("---\nname: \"A\"\nname: \"B\"\n---\n");
        expect_err("---\nshowInLauncher: false\n---\n");
        expect_err("---\nunknown: \"value\"\n---\n");
        expect_err("---\ncategory: \"Work\"\n---\n");
        expect_err("---\nshow_in_launcher: true\n---\n");
        expect_err("---\nshow_hud: true\n---\n");
        expect_err("---\r\nname: \"Safe\r\nenabled: false\"\r\n---\r\nBody");
    }
}
