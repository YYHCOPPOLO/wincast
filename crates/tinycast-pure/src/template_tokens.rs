//! Token parser for snippet/quicklink templates.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Modifier {
    Uppercase,
    Lowercase,
    Trim,
    PercentEncode,
    JsonStringify,
    Raw,
}

impl Modifier {
    fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "uppercase" => Self::Uppercase,
            "lowercase" => Self::Lowercase,
            "trim" => Self::Trim,
            "percent-encode" => Self::PercentEncode,
            "json-stringify" => Self::JsonStringify,
            "raw" => Self::Raw,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateTimeKind {
    Date,
    Time,
    DateTime,
    Weekday,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetUnit {
    Minute,
    Hour,
    Day,
    Month,
    Year,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateOffset {
    pub unit: OffsetUnit,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DateTimeToken {
    pub kind: DateTimeKind,
    pub offsets: Vec<DateOffset>,
    pub format: Option<String>,
    pub locale: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgumentToken {
    pub name: String,
    pub options: Vec<String>,
    pub default: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment {
    Literal(String),
    Clipboard {
        offset: usize,
        modifiers: Vec<Modifier>,
    },
    Selection {
        modifiers: Vec<Modifier>,
    },
    DateTime {
        token: DateTimeToken,
        modifiers: Vec<Modifier>,
    },
    Uuid {
        modifiers: Vec<Modifier>,
    },
    Argument {
        token: ArgumentToken,
        source: String,
        modifiers: Vec<Modifier>,
    },
    Cursor,
    SnippetRef {
        key: String,
        source: String,
    },
}

pub fn parse_segments(source: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut i = 0;
    while i < source.len() {
        let rest = &source[i..];
        let Some(rel) = rest.find('{') else {
            segments.push(Segment::Literal(source[i..].to_string()));
            break;
        };
        if rel > 0 {
            segments.push(Segment::Literal(source[i..i + rel].to_string()));
        }
        let open = i + rel;
        let after_open = open + 1;
        let Some(rel_close) = source[after_open..].find('}') else {
            segments.push(Segment::Literal(source[open..].to_string()));
            break;
        };
        let close = after_open + rel_close;
        let end = close + 1;
        let raw_token = &source[open..end];
        let body = &source[after_open..close];
        if let Some(segment) = segment_for(body, raw_token) {
            segments.push(segment);
            i = end;
        } else {
            segments.push(Segment::Literal("{".into()));
            i = after_open;
        }
    }
    merge_literals(segments)
}

fn merge_literals(segments: Vec<Segment>) -> Vec<Segment> {
    let mut out = Vec::new();
    for segment in segments {
        if let Segment::Literal(text) = &segment {
            if let Some(Segment::Literal(prev)) = out.last_mut() {
                prev.push_str(text);
                continue;
            }
        }
        out.push(segment);
    }
    out
}

fn segment_for(body: &str, source: &str) -> Option<Segment> {
    let parts = split_on_pipes(body)?;
    let head = parts.first()?.as_str();
    let mut modifiers = Vec::new();
    for raw in parts.iter().skip(1) {
        modifiers.push(Modifier::parse(raw)?);
    }

    if head == "cursor" {
        return if modifiers.is_empty() {
            Some(Segment::Cursor)
        } else {
            None
        };
    }
    if let Some(key) = head.strip_prefix("snippet:") {
        let key = key.trim();
        if key.is_empty() || !modifiers.is_empty() {
            return None;
        }
        return Some(Segment::SnippetRef {
            key: key.to_string(),
            source: source.to_string(),
        });
    }

    let token = parse_token(head)?;
    match token.command.as_str() {
        "clipboard" => {
            let offset = int_parameter(&token, "offset", 0)?;
            if offset < 0 || !token.has_only(&["offset"]) {
                return None;
            }
            Some(Segment::Clipboard {
                offset: offset as usize,
                modifiers,
            })
        }
        "selection" | "selectedtext" => {
            if !token.parameters.is_empty() {
                return None;
            }
            Some(Segment::Selection { modifiers })
        }
        "uuid" => {
            if !token.parameters.is_empty() {
                return None;
            }
            Some(Segment::Uuid { modifiers })
        }
        "date" | "time" | "datetime" | "day" => {
            let date_time = parse_date_time(&token)?;
            Some(Segment::DateTime {
                token: date_time,
                modifiers,
            })
        }
        "argument" => {
            let argument = parse_argument(&token)?;
            Some(Segment::Argument {
                token: argument,
                source: source.to_string(),
                modifiers,
            })
        }
        "snippet" => {
            let name = token.parameters.get("name")?.trim();
            if name.is_empty() || !token.has_only(&["name"]) || !modifiers.is_empty() {
                return None;
            }
            Some(Segment::SnippetRef {
                key: name.to_string(),
                source: source.to_string(),
            })
        }
        _ => None,
    }
}

struct ParsedToken {
    command: String,
    parameters: std::collections::HashMap<String, String>,
}

impl ParsedToken {
    fn has_only(&self, allowed: &[&str]) -> bool {
        self.parameters
            .keys()
            .all(|key| allowed.iter().copied().any(|a| a == key))
    }
}

fn int_parameter(token: &ParsedToken, key: &str, fallback: i32) -> Option<i32> {
    match token.parameters.get(key) {
        None => Some(fallback),
        Some(raw) => raw.parse().ok(),
    }
}

fn parse_date_time(token: &ParsedToken) -> Option<DateTimeToken> {
    if !token.has_only(&["offset", "format", "locale"]) {
        return None;
    }
    let format = token.parameters.get("format").cloned();
    let locale = token.parameters.get("locale").cloned();
    if format.is_some() && locale.is_some() {
        return None;
    }
    if format.as_ref().is_some_and(|s| s.is_empty()) {
        return None;
    }
    if locale.as_ref().is_some_and(|s| s.is_empty()) {
        return None;
    }
    let offsets = match token.parameters.get("offset") {
        Some(raw) => parse_offsets(raw)?,
        None => Vec::new(),
    };
    let kind = match token.command.as_str() {
        "date" => DateTimeKind::Date,
        "time" => DateTimeKind::Time,
        "datetime" => DateTimeKind::DateTime,
        _ => DateTimeKind::Weekday,
    };
    Some(DateTimeToken {
        kind,
        offsets,
        format,
        locale,
    })
}

fn parse_offsets(raw: &str) -> Option<Vec<DateOffset>> {
    let pieces: Vec<&str> = raw.split_whitespace().collect();
    if pieces.is_empty() {
        return None;
    }
    let mut offsets = Vec::new();
    for piece in pieces {
        let mut chars = piece.chars();
        let unit_ch = chars.next_back()?;
        let unit = match unit_ch {
            'm' => OffsetUnit::Minute,
            'h' => OffsetUnit::Hour,
            'd' => OffsetUnit::Day,
            'M' => OffsetUnit::Month,
            'y' => OffsetUnit::Year,
            _ => return None,
        };
        let amount: String = chars.collect();
        if amount.is_empty() {
            return None;
        }
        let value: i32 = amount.parse().ok()?;
        offsets.push(DateOffset { unit, value });
    }
    Some(offsets)
}

fn parse_argument(token: &ParsedToken) -> Option<ArgumentToken> {
    if !token.has_only(&["name", "default", "options"]) {
        return None;
    }
    let name = token
        .parameters
        .get("name")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Argument".into());
    let options = match token.parameters.get("options") {
        None => Vec::new(),
        Some(raw) => {
            let options: Vec<String> = raw
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if options.is_empty() {
                return None;
            }
            options
        }
    };
    Some(ArgumentToken {
        name,
        options,
        default: token.parameters.get("default").cloned(),
    })
}

fn split_on_pipes(body: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escaped = false;
    for ch in body.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => {
                current.push(ch);
                escaped = true;
            }
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            '|' if !in_quotes => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if in_quotes || escaped {
        return None;
    }
    parts.push(current.trim().to_string());
    if parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    Some(parts)
}

fn parse_token(head: &str) -> Option<ParsedToken> {
    let mut remainder = head.trim_start();
    let command_len = remainder
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(remainder.len());
    if command_len == 0 {
        return None;
    }
    let command = remainder[..command_len].to_ascii_lowercase();
    remainder = remainder[command_len..].trim_start();

    let mut parameters = std::collections::HashMap::new();
    while !remainder.is_empty() {
        remainder = remainder.trim_start();
        if remainder.is_empty() {
            break;
        }
        let key_len = remainder
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(remainder.len());
        if key_len == 0 {
            return None;
        }
        let key = remainder[..key_len].to_string();
        remainder = remainder[key_len..].trim_start();
        if !remainder.starts_with('=') {
            return None;
        }
        remainder = remainder[1..].trim_start();
        let value = if remainder.starts_with('"') {
            remainder = &remainder[1..];
            let (decoded, rest) = decode_quoted(remainder)?;
            remainder = rest;
            decoded
        } else {
            let val_len = remainder
                .find(char::is_whitespace)
                .unwrap_or(remainder.len());
            if val_len == 0 {
                return None;
            }
            let value = remainder[..val_len].to_string();
            remainder = &remainder[val_len..];
            value
        };
        if parameters.insert(key, value).is_some() {
            return None;
        }
    }
    Some(ParsedToken {
        command,
        parameters,
    })
}

fn decode_quoted(input: &str) -> Option<(String, &str)> {
    let mut decoded = String::new();
    let mut escaped = false;
    let mut chars = input.char_indices();
    while let Some((idx, ch)) = chars.next() {
        if escaped {
            match ch {
                '\\' => decoded.push('\\'),
                '"' => decoded.push('"'),
                'n' => decoded.push('\n'),
                'r' => decoded.push('\r'),
                't' => decoded.push('\t'),
                _ => return None,
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            let rest = &input[idx + ch.len_utf8()..];
            return Some((decoded, rest));
        } else {
            decoded.push(ch);
        }
    }
    None
}
