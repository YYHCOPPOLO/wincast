//! Shared snippet/quicklink template engine. Clock, locale, clipboard, and UUID are injected.

use std::collections::{HashMap, HashSet};

use unicode_segmentation::UnicodeSegmentation;

use crate::template_tokens::{
    parse_segments, ArgumentToken, DateOffset, DateTimeKind, DateTimeToken, Modifier, OffsetUnit,
    Segment,
};

#[derive(Clone)]
pub struct ExpandContext {
    pub clipboard: Vec<String>,
    pub selection: Option<String>,
    pub now: i64,
    pub locale: String,
    pub tz: String,
    pub uuid: fn() -> String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentSpec {
    pub name: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandOutput {
    pub text: String,
    pub cursor: Option<usize>,
    pub arguments: Vec<ArgumentSpec>,
}

/// Snippet record the engine can nest into. Identity is `id` (path).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateSnippet {
    pub id: String,
    pub name: String,
    pub keyword: Option<String>,
    pub enabled: bool,
    pub body: String,
}

const MAX_REFERENCE_DEPTH: usize = 5;

pub fn expand(input: &str, ctx: &ExpandContext, args: &HashMap<String, String>) -> ExpandOutput {
    expand_with(input, ctx, args, false)
}

pub fn expand_with(
    input: &str,
    ctx: &ExpandContext,
    args: &HashMap<String, String>,
    auto_percent: bool,
) -> ExpandOutput {
    expand_inner(
        input,
        ctx,
        args,
        &[],
        auto_percent,
        0,
        &HashSet::new(),
        false,
    )
}

/// Destination expansion: `{cursor}` and `{snippet:…}` stay literal.
pub fn expand_destination_template(
    input: &str,
    ctx: &ExpandContext,
    args: &HashMap<String, String>,
    auto_percent: bool,
) -> ExpandOutput {
    expand_inner(
        input,
        ctx,
        args,
        &[],
        auto_percent,
        0,
        &HashSet::new(),
        true,
    )
}

pub fn expand_snippet(
    input: &str,
    ctx: &ExpandContext,
    args: &HashMap<String, String>,
    snippets: &[TemplateSnippet],
    root_id: Option<&str>,
) -> ExpandOutput {
    let mut visited = HashSet::new();
    if let Some(id) = root_id {
        visited.insert(id.to_string());
    }
    expand_inner(input, ctx, args, snippets, false, 0, &visited, false)
}

pub fn uses_selection(input: &str) -> bool {
    parse_segments(input)
        .iter()
        .any(|s| matches!(s, Segment::Selection { .. }))
}

fn expand_inner(
    input: &str,
    ctx: &ExpandContext,
    args: &HashMap<String, String>,
    snippets: &[TemplateSnippet],
    auto_percent: bool,
    depth: usize,
    visited: &HashSet<String>,
    destination: bool,
) -> ExpandOutput {
    let mut text = String::new();
    let mut cursor = None;
    let mut arguments = Vec::new();
    let mut argument_names = HashSet::new();
    let mut sorted = snippets.to_vec();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));

    for segment in parse_segments(input) {
        match segment {
            Segment::Literal(value) => text.push_str(&value),
            Segment::Clipboard { offset, modifiers } => {
                let value = ctx.clipboard.get(offset).cloned().unwrap_or_default();
                text.push_str(&apply_modifiers(&modifiers, &value, auto_percent));
            }
            Segment::Selection { modifiers } => {
                let value = ctx.selection.clone().unwrap_or_default();
                text.push_str(&apply_modifiers(&modifiers, &value, auto_percent));
            }
            Segment::DateTime { token, modifiers } => {
                let value = format_date_time(&token, ctx);
                text.push_str(&apply_modifiers(&modifiers, &value, auto_percent));
            }
            Segment::Uuid { modifiers } => {
                let value = (ctx.uuid)();
                text.push_str(&apply_modifiers(&modifiers, &value, auto_percent));
            }
            Segment::Argument {
                token,
                source,
                modifiers,
            } => match args.get(&token.name).cloned().or(token.default.clone()) {
                Some(value) => {
                    text.push_str(&apply_modifiers(&modifiers, &value, auto_percent));
                }
                None => {
                    text.push_str(&source);
                    add_missing(&mut arguments, &mut argument_names, &token);
                }
            },
            Segment::Cursor => {
                if destination {
                    text.push_str("{cursor}");
                } else if cursor.is_none() {
                    cursor = Some(grapheme_count(&text));
                }
            }
            Segment::SnippetRef { key, source } => {
                if destination {
                    text.push_str(&source);
                    continue;
                }
                let resolved = resolve_reference(&key, &sorted);
                let allowed = depth < MAX_REFERENCE_DEPTH
                    && resolved
                        .as_ref()
                        .is_some_and(|s| !visited.contains(&s.id));
                if !allowed {
                    text.push_str(&source);
                    continue;
                }
                let target = resolved.unwrap();
                let mut nested_visited = visited.clone();
                nested_visited.insert(target.id.clone());
                let nested = expand_inner(
                    &target.body,
                    ctx,
                    args,
                    snippets,
                    auto_percent,
                    depth + 1,
                    &nested_visited,
                    false,
                );
                let insertion = grapheme_count(&text);
                if cursor.is_none() {
                    if let Some(nested_cursor) = nested.cursor {
                        cursor = Some(insertion + nested_cursor);
                    }
                }
                text.push_str(&nested.text);
                for argument in nested.arguments {
                    if argument_names.insert(argument.name.clone()) {
                        arguments.push(argument);
                    }
                }
            }
        }
    }

    ExpandOutput {
        text,
        cursor,
        arguments,
    }
}

fn add_missing(
    arguments: &mut Vec<ArgumentSpec>,
    names: &mut HashSet<String>,
    token: &ArgumentToken,
) {
    if names.insert(token.name.clone()) {
        arguments.push(ArgumentSpec {
            name: token.name.clone(),
            options: token.options.clone(),
        });
    }
}

fn resolve_reference<'a>(key: &str, snippets: &'a [TemplateSnippet]) -> Option<&'a TemplateSnippet> {
    let normalized = normalize_reference(key);
    let candidates: Vec<&TemplateSnippet> = snippets.iter().filter(|s| s.enabled).collect();
    if let Some(found) = candidates
        .iter()
        .copied()
        .find(|s| normalize_reference(&s.name) == normalized)
    {
        return Some(found);
    }
    candidates.into_iter().find(|s| {
        s.keyword
            .as_ref()
            .is_some_and(|k| normalize_reference(k) == normalized)
    })
}

fn normalize_reference(value: &str) -> String {
    value.to_lowercase()
}

fn apply_modifiers(modifiers: &[Modifier], value: &str, auto_percent: bool) -> String {
    let mut modified = value.to_string();
    for modifier in modifiers {
        modified = match modifier {
            Modifier::Uppercase => modified.to_uppercase(),
            Modifier::Lowercase => modified.to_lowercase(),
            Modifier::Trim => modified.trim().to_string(),
            Modifier::PercentEncode => percent_encode(&modified),
            Modifier::JsonStringify => json_escape(&modified),
            Modifier::Raw => modified,
        };
    }
    if auto_percent
        && !modifiers
            .iter()
            .any(|m| matches!(m, Modifier::Raw | Modifier::PercentEncode))
    {
        percent_encode(&modified)
    } else {
        modified
    }
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | '~') {
            out.push(ch);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn json_escape(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn grapheme_count(s: &str) -> usize {
    s.graphemes(true).count()
}

fn format_date_time(token: &DateTimeToken, ctx: &ExpandContext) -> String {
    let tz = tz_offset_seconds(&ctx.tz);
    let unix = apply_offsets(ctx.now, tz, &token.offsets);
    let locale = token.locale.as_deref().unwrap_or(&ctx.locale);
    let civil = civil_at(unix, tz);
    if let Some(fmt) = &token.format {
        return apply_pattern(fmt, &civil, locale);
    }
    match token.kind {
        DateTimeKind::Date => medium_date(&civil, locale),
        DateTimeKind::Time => short_time(&civil, locale),
        DateTimeKind::DateTime => format!(
            "{} {} {}",
            medium_date(&civil, locale),
            datetime_joiner(locale),
            short_time(&civil, locale)
        ),
        DateTimeKind::Weekday => weekday_name(civil.weekday(), locale, true),
    }
}

fn apply_offsets(mut unix: i64, tz: i32, offsets: &[DateOffset]) -> i64 {
    for offset in offsets {
        unix = match offset.unit {
            OffsetUnit::Minute => unix.saturating_add(i64::from(offset.value) * 60),
            OffsetUnit::Hour => unix.saturating_add(i64::from(offset.value) * 3600),
            OffsetUnit::Day => unix.saturating_add(i64::from(offset.value) * 86_400),
            OffsetUnit::Month => add_months(unix, tz, offset.value),
            OffsetUnit::Year => add_months(unix, tz, offset.value.saturating_mul(12)),
        };
    }
    unix
}

fn tz_offset_seconds(tz: &str) -> i32 {
    let t = tz.trim();
    if t.eq_ignore_ascii_case("utc") || t.eq_ignore_ascii_case("gmt") || t == "Z" {
        return 0;
    }
    let bytes = t.as_bytes();
    if bytes.is_empty() {
        return 0;
    }
    let (sign, rest) = match bytes[0] {
        b'+' => (1, &t[1..]),
        b'-' => (-1, &t[1..]),
        _ => return 0,
    };
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    let (hh, mm) = match digits.len() {
        1 | 2 => (digits.parse::<i32>().unwrap_or(0), 0),
        4 => (
            digits[..2].parse::<i32>().unwrap_or(0),
            digits[2..].parse::<i32>().unwrap_or(0),
        ),
        _ => return 0,
    };
    sign * (hh * 3600 + mm * 60)
}

#[derive(Clone, Copy)]
struct Civil {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

impl Civil {
    fn weekday(self) -> u32 {
        // Sunday = 0 … Saturday = 6. Unix day 0 is Thursday.
        let unix = days_from_civil(self.year, self.month, self.day) * 86_400;
        ((unix.div_euclid(86_400) + 4).rem_euclid(7)) as u32
    }
}

fn civil_at(unix: i64, tz: i32) -> Civil {
    let local = unix.saturating_add(i64::from(tz));
    let days = local.div_euclid(86_400);
    let sod = local.rem_euclid(86_400) as u32;
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        hour: sod / 3600,
        minute: (sod % 3600) / 60,
        second: sod % 60,
    }
}

fn add_months(unix: i64, tz: i32, months: i32) -> i64 {
    let c = civil_at(unix, tz);
    let month0 = i64::from(c.year) * 12 + i64::from(c.month) - 1 + i64::from(months);
    let year = month0.div_euclid(12) as i32;
    let month = (month0.rem_euclid(12) as u32) + 1;
    let day = c.day.min(days_in_month(year, month));
    let local_midnight = days_from_civil(year, month, day) * 86_400;
    local_midnight + i64::from(c.hour * 3600 + c.minute * 60 + c.second) - i64::from(tz)
}

fn lang(locale: &str) -> &str {
    let cut = locale
        .find(['-', '_'])
        .unwrap_or_else(|| locale.len().min(2));
    let head = if cut == 0 { locale } else { &locale[..cut] };
    if head.len() >= 2 {
        &head[..2]
    } else {
        head
    }
}

fn is_french(locale: &str) -> bool {
    lang(locale).eq_ignore_ascii_case("fr")
}

fn datetime_joiner(locale: &str) -> &'static str {
    if is_french(locale) {
        "à"
    } else {
        "at"
    }
}

fn medium_date(c: &Civil, locale: &str) -> String {
    if is_french(locale) {
        format!("{} {} {}", c.day, month_name(c.month, locale, false), c.year)
    } else {
        format!("{} {}, {}", month_name(c.month, locale, false), c.day, c.year)
    }
}

fn short_time(c: &Civil, locale: &str) -> String {
    if is_french(locale) {
        format!("{:02}:{:02}", c.hour, c.minute)
    } else {
        let (hour12, ampm) = hour12(c.hour);
        format!("{hour12}:{:02}\u{202F}{ampm}", c.minute)
    }
}

fn hour12(hour: u32) -> (u32, &'static str) {
    let ampm = if hour < 12 { "AM" } else { "PM" };
    let h = match hour % 12 {
        0 => 12,
        h => h,
    };
    (h, ampm)
}

fn month_name(month: u32, locale: &str, full: bool) -> &'static str {
    let idx = month.saturating_sub(1).min(11) as usize;
    if is_french(locale) {
        if full {
            FR_MONTHS_FULL[idx]
        } else {
            FR_MONTHS_ABBR[idx]
        }
    } else if full {
        EN_MONTHS_FULL[idx]
    } else {
        EN_MONTHS_ABBR[idx]
    }
}

fn weekday_name(weekday: u32, locale: &str, full: bool) -> String {
    let idx = (weekday as usize) % 7;
    if is_french(locale) {
        if full {
            FR_WEEKDAYS_FULL[idx]
        } else {
            FR_WEEKDAYS_ABBR[idx]
        }
    } else if full {
        EN_WEEKDAYS_FULL[idx]
    } else {
        EN_WEEKDAYS_ABBR[idx]
    }
    .to_string()
}

fn apply_pattern(fmt: &str, c: &Civil, locale: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' {
            i += 1;
            if i < chars.len() && chars[i] == '\'' {
                out.push('\'');
                i += 1;
                continue;
            }
            while i < chars.len() {
                if chars[i] == '\'' {
                    i += 1;
                    if i < chars.len() && chars[i] == '\'' {
                        out.push('\'');
                        i += 1;
                    } else {
                        break;
                    }
                } else {
                    out.push(chars[i]);
                    i += 1;
                }
            }
            continue;
        }
        if ch.is_ascii_alphabetic() {
            let mut n = 1;
            while i + n < chars.len() && chars[i + n] == ch {
                n += 1;
            }
            out.push_str(&pattern_field(ch, n, c, locale));
            i += n;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn pattern_field(letter: char, count: usize, c: &Civil, locale: &str) -> String {
    match letter {
        'y' => {
            let year = c.year.unsigned_abs();
            if count == 2 {
                format!("{:02}", year % 100)
            } else {
                format!("{year:0width$}", width = count.max(4))
            }
        }
        'M' => match count {
            1 => c.month.to_string(),
            2 => format!("{:02}", c.month),
            3 => month_name(c.month, locale, false).to_string(),
            _ => month_name(c.month, locale, true).to_string(),
        },
        'd' => {
            if count == 1 {
                c.day.to_string()
            } else {
                format!("{:02}", c.day)
            }
        }
        'E' => weekday_name(c.weekday(), locale, count >= 4),
        'H' => {
            if count == 1 {
                c.hour.to_string()
            } else {
                format!("{:02}", c.hour)
            }
        }
        'h' => {
            let (h, _) = hour12(c.hour);
            if count == 1 {
                h.to_string()
            } else {
                format!("{h:02}")
            }
        }
        'm' => {
            if count == 1 {
                c.minute.to_string()
            } else {
                format!("{:02}", c.minute)
            }
        }
        's' => {
            if count == 1 {
                c.second.to_string()
            } else {
                format!("{:02}", c.second)
            }
        }
        'a' => {
            let (_, ampm) = hour12(c.hour);
            ampm.to_string()
        }
        _ => letter.to_string().repeat(count),
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let mut y = y as i64;
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp as u64 + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

const EN_MONTHS_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const EN_MONTHS_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const FR_MONTHS_ABBR: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
    "déc.",
];
const FR_MONTHS_FULL: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];
const EN_WEEKDAYS_ABBR: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const EN_WEEKDAYS_FULL: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const FR_WEEKDAYS_ABBR: [&str; 7] = ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."];
const FR_WEEKDAYS_FULL: [&str; 7] = [
    "dimanche",
    "lundi",
    "mardi",
    "mercredi",
    "jeudi",
    "vendredi",
    "samedi",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn ctx() -> ExpandContext {
        ExpandContext {
            clipboard: vec!["Hi".into()],
            selection: None,
            now: 0,
            locale: "en".into(),
            tz: "UTC".into(),
            uuid: || "u".into(),
        }
    }

    fn posix_ctx() -> ExpandContext {
        ExpandContext {
            clipboard: vec!["{date} 📋".into()],
            selection: Some("{cursor} selected".into()),
            now: unix(2026, 7, 24, 13, 5, 0),
            locale: "en_US_POSIX".into(),
            tz: "UTC".into(),
            uuid: || "u".into(),
        }
    }

    fn unix(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> i64 {
        days_from_civil(y, m, d) * 86_400 + i64::from(h * 3600 + min * 60 + s)
    }

    fn sn(id: &str, name: &str, body: &str) -> TemplateSnippet {
        TemplateSnippet {
            id: id.into(),
            name: name.into(),
            keyword: None,
            enabled: true,
            body: body.into(),
        }
    }

    #[test]
    fn clipboard_and_cursor_and_unknown_left_in_place() {
        let o = expand("X{clipboard}{cursor}Y{nope}", &ctx(), &Default::default());
        assert_eq!(o.text, "XHiY{nope}");
        assert_eq!(o.cursor, Some(3));
    }

    #[test]
    fn default_date_and_time_use_injected_clock() {
        let o = expand("{date}|{time}", &posix_ctx(), &Default::default());
        assert_eq!(o.text, "Jul 24, 2026|1:05\u{202F}PM");
    }

    #[test]
    fn missing_arguments_unique_and_ordered() {
        let t = "{argument name=\"First\"}|{argument}|{argument name=\"First\"}";
        let o = expand(t, &posix_ctx(), &Default::default());
        assert_eq!(
            o.arguments
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>(),
            ["First", "Argument"]
        );
        assert!(o
            .text
            .ends_with("{argument name=\"First\"}|{argument}|{argument name=\"First\"}"));
    }

    #[test]
    fn clipboard_selection_and_arguments_are_literal() {
        let t = "C:{clipboard}|S:{selection}|D:{date format=\"yyyy-MM-dd HH:mm\"}|{argument name=\"First\"}|{argument}|{argument name=\"First\"}";
        let mut args = HashMap::new();
        args.insert("First".into(), "{clipboard}".into());
        args.insert("Argument".into(), "{cursor}".into());
        let o = expand(t, &posix_ctx(), &args);
        assert_eq!(
            o.text,
            "C:{date} 📋|S:{cursor} selected|D:2026-07-24 13:05|{clipboard}|{cursor}|{clipboard}"
        );
        assert!(o.cursor.is_none());
        assert!(o.arguments.is_empty());
    }

    #[test]
    fn literal_braces_do_not_mask_nested_tokens() {
        let o = expand(
            "{\"generated\":\"{date}\"}|struct { value: {time} }",
            &posix_ctx(),
            &Default::default(),
        );
        assert_eq!(
            o.text,
            "{\"generated\":\"Jul 24, 2026\"}|struct { value: 1:05\u{202F}PM }"
        );
    }

    #[test]
    fn duplicate_name_references_resolve_by_path() {
        let z = sn("/tmp/z-child.md", "Child", "Z");
        let mut a = sn("/tmp/a-child.md", "Child", "A");
        a.keyword = Some("!CHILD".into());
        let mut key = sn("/tmp/keyword.md", "Other", "K");
        key.keyword = Some("!Key".into());
        let root = sn(
            "/tmp/references.md",
            "References",
            "{snippet:cHiLd}|{snippet:!kEy}|{snippet:missing}",
        );
        let o = expand_snippet(
            &root.body,
            &posix_ctx(),
            &Default::default(),
            &[z, key, root.clone(), a],
            Some(&root.id),
        );
        assert_eq!(o.text, "A|K|{snippet:missing}");
    }

    #[test]
    fn disabled_snippet_cannot_be_referenced() {
        let mut child = sn("/tmp/disabled-child.md", "Disabled", "Secret");
        child.keyword = Some("!disabled".into());
        child.enabled = false;
        let root = sn(
            "/tmp/disabled-references.md",
            "Disabled References",
            "{snippet:Disabled}|{snippet:!disabled}",
        );
        let o = expand_snippet(
            &root.body,
            &posix_ctx(),
            &Default::default(),
            &[child, root.clone()],
            Some(&root.id),
        );
        assert_eq!(o.text, "{snippet:Disabled}|{snippet:!disabled}");
    }

    #[test]
    fn first_cursor_includes_nested_and_uses_graphemes() {
        let child = sn("/tmp/cursor-child.md", "Cursor Child", "👨‍👩‍👧‍👦{cursor}é{cursor}");
        let root = sn(
            "/tmp/cursor-root.md",
            "Cursor Root",
            "🙂{snippet:Cursor Child}終{cursor}",
        );
        let o = expand_snippet(
            &root.body,
            &posix_ctx(),
            &Default::default(),
            &[root.clone(), child],
            Some(&root.id),
        );
        assert_eq!(o.text, "🙂👨‍👩‍👧‍👦é終");
        assert_eq!(o.cursor, Some(2));
    }

    #[test]
    fn nested_arguments_follow_appearance_order() {
        let nested = sn(
            "/tmp/nested-arguments.md",
            "Nested Arguments",
            "{argument name=\"Nested\"}|{argument name=\"Root\"}",
        );
        let root = sn(
            "/tmp/argument-root.md",
            "Argument Root",
            "{argument name=\"Root\"}|{snippet:Nested Arguments}|{argument name=\"Last\"}",
        );
        let o = expand_snippet(
            &root.body,
            &posix_ctx(),
            &Default::default(),
            &[root.clone(), nested],
            Some(&root.id),
        );
        let names: Vec<&str> = o.arguments.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Root", "Nested", "Last"]);
    }

    #[test]
    fn cycles_leave_the_token_visible() {
        let a = sn("/tmp/cycle-a.md", "A", "{snippet:B}");
        let b = sn("/tmp/cycle-b.md", "B", "{snippet:A}");
        let o = expand_snippet(
            &a.body,
            &posix_ctx(),
            &Default::default(),
            &[a.clone(), b],
            Some(&a.id),
        );
        assert_eq!(o.text, "{snippet:A}");
    }

    #[test]
    fn reference_depth_limit_leaves_token() {
        let records: Vec<TemplateSnippet> = (0..=6)
            .map(|i| {
                let body = if i == 6 {
                    "End".into()
                } else {
                    format!("{{snippet:S{}}}", i + 1)
                };
                sn(&format!("/tmp/depth-{i}.md"), &format!("S{i}"), &body)
            })
            .collect();
        let o = expand_snippet(
            &records[0].body,
            &posix_ctx(),
            &Default::default(),
            &records,
            Some(&records[0].id),
        );
        assert_eq!(o.text, "{snippet:S6}");
    }

    #[test]
    fn dynamic_placeholders() {
        static N: AtomicU32 = AtomicU32::new(0);
        fn next_uuid() -> String {
            let n = N.fetch_add(1, Ordering::SeqCst) + 1;
            format!("uuid-{n}")
        }
        N.store(0, Ordering::SeqCst);
        let ctx = ExpandContext {
            clipboard: vec!["  newest  ".into(), "older".into(), "oldest".into()],
            selection: Some("picked".into()),
            now: unix(2026, 7, 24, 13, 5, 0),
            locale: "en_US_POSIX".into(),
            tz: "UTC".into(),
            uuid: next_uuid,
        };
        let e = |t: &str| expand(t, &ctx, &Default::default()).text;
        assert_eq!(e("{datetime}"), "Jul 24, 2026 at 1:05\u{202F}PM");
        assert_eq!(e("{day}"), "Friday");
        assert_eq!(e("{date offset=\"+1d\"}"), "Jul 25, 2026");
        assert_eq!(e("{day offset=-3d}"), "Tuesday");
        assert_eq!(e("{date offset=\"+2y +5M\"}"), "Dec 24, 2028");
        assert_eq!(e("{time offset=\"+3h +30m\"}"), "4:35\u{202F}PM");
        assert_eq!(e("{date offset=\"+1w\"}"), "{date offset=\"+1w\"}");
        assert_eq!(e("{date offset=\"d\"}"), "{date offset=\"d\"}");
        assert_eq!(e("{date locale=\"fr-FR\"}"), "24 juil. 2026");
        assert_eq!(
            e("{date format=\"yyyy\" locale=\"fr-FR\"}"),
            "{date format=\"yyyy\" locale=\"fr-FR\"}"
        );
        assert_eq!(e("{date offset=\"-1d\" format=\"yyyy-MM-dd\"}"), "2026-07-23");
        assert_eq!(e("{uuid}|{uuid}"), "uuid-1|uuid-2");
        assert_eq!(e("{clipboard}"), "  newest  ");
        assert_eq!(e("{clipboard offset=1}|{clipboard offset=2}"), "older|oldest");
        assert_eq!(e("{clipboard offset=9}"), "");
        assert_eq!(e("{clipboard offset=-1}"), "{clipboard offset=-1}");
        assert_eq!(
            e("{selection | uppercase}|{selection | lowercase}"),
            "PICKED|picked"
        );
        assert_eq!(e("{clipboard | trim}"), "newest");
        assert_eq!(e("{clipboard | trim | uppercase}"), "NEWEST");
        let mut args = HashMap::new();
        args.insert("U".into(), "a b/c?d&e=f~g-h".into());
        assert_eq!(
            expand("{argument name=\"U\" | percent-encode}", &ctx, &args).text,
            "a%20b%2Fc%3Fd%26e%3Df~g-h"
        );
        args.insert("J".into(), "a\"b\\c\nd".into());
        assert_eq!(
            expand("{argument name=\"J\" | json-stringify}", &ctx, &args).text,
            "a\\\"b\\\\c\\nd"
        );
        assert_eq!(e("{clipboard | raw}"), "  newest  ");
        assert_eq!(e("{clipboard | shout}"), "{clipboard | shout}");
        assert_eq!(e("{cursor | uppercase}"), "{cursor | uppercase}");
        assert_eq!(e("{date format=\"yyyy|MM\"}"), "2026|07");
        let defaulted = expand(
            "{argument name=\"Tone\" default=\"happy\"}",
            &ctx,
            &Default::default(),
        );
        assert_eq!(defaulted.text, "happy");
        assert!(defaulted.arguments.is_empty());
        args.insert("Tone".into(), "sad".into());
        assert_eq!(
            expand(
                "{argument name=\"Tone\" default=\"happy\"}",
                &ctx,
                &args
            )
            .text,
            "sad"
        );
        let optioned = expand(
            "{argument name=\"Tone\" options=\"happy, sad, professional\"}",
            &ctx,
            &Default::default(),
        );
        assert_eq!(
            optioned.arguments,
            [ArgumentSpec {
                name: "Tone".into(),
                options: vec!["happy".into(), "sad".into(), "professional".into()],
            }]
        );
        assert_eq!(
            e("{argument name=\"Tone\" options=\", \"}"),
            "{argument name=\"Tone\" options=\", \"}"
        );
        assert_eq!(e("{weather}"), "{weather}");
        assert_eq!(e("{date style=\"long\"}"), "{date style=\"long\"}");
        assert_eq!(
            e("{date offset=\"+1d\" offset=\"+2d\"}"),
            "{date offset=\"+1d\" offset=\"+2d\"}"
        );
        assert_eq!(e("{date format=\"yyyy}"), "{date format=\"yyyy}");
        assert_eq!(e("{uuid offset=1}"), "{uuid offset=1}");
    }

    #[test]
    fn snippet_name_parameter_matches_colon_form() {
        let child = sn("/tmp/ph-child.md", "Child", "nested");
        let by_name = sn("/tmp/ph-name.md", "ByName", "{snippet name=\"Child\"}");
        let by_colon = sn("/tmp/ph-colon.md", "ByColon", "{snippet:Child}");
        let pool = [child, by_name.clone(), by_colon.clone()];
        assert_eq!(
            expand_snippet(
                &by_name.body,
                &posix_ctx(),
                &Default::default(),
                &pool,
                Some(&by_name.id)
            )
            .text,
            "nested"
        );
        assert_eq!(
            expand_snippet(
                &by_colon.body,
                &posix_ctx(),
                &Default::default(),
                &pool,
                Some(&by_colon.id)
            )
            .text,
            "nested"
        );
    }

    #[test]
    fn percent_encoding_and_selection_alias() {
        let ctx = ExpandContext {
            clipboard: vec!["a b&c".into()],
            selection: Some("a b&c".into()),
            now: unix(2026, 7, 24, 0, 0, 0),
            locale: "en_US_POSIX".into(),
            tz: "UTC".into(),
            uuid: || "u".into(),
        };
        assert_eq!(expand("q={clipboard}", &ctx, &Default::default()).text, "q=a b&c");
        assert_eq!(
            expand("{snippet:Child}", &ctx, &Default::default()).text,
            "{snippet:Child}"
        );
        assert_eq!(
            expand("{argument name=\"Repository\"}", &ctx, &Default::default())
                .arguments
                .first()
                .map(|a| a.name.as_str()),
            Some("Repository")
        );
        assert_eq!(
            expand_with(
                "https://x.com/?q={clipboard}",
                &ctx,
                &Default::default(),
                true
            )
            .text,
            "https://x.com/?q=a%20b%26c"
        );
        assert_eq!(
            expand_with(
                "https://x.com/a b?q={selection}",
                &ctx,
                &Default::default(),
                true
            )
            .text,
            "https://x.com/a b?q=a%20b%26c"
        );
        assert_eq!(
            expand_with("{clipboard | uppercase}", &ctx, &Default::default(), true).text,
            "A%20B%26C"
        );
        assert_eq!(
            expand_with("{clipboard | raw}", &ctx, &Default::default(), true).text,
            "a b&c"
        );
        assert_eq!(
            expand_with("{clipboard | percent-encode}", &ctx, &Default::default(), true).text,
            "a%20b%26c"
        );
        let mut args = HashMap::new();
        args.insert("A".into(), "x y".into());
        assert_eq!(
            expand_with("{argument name=\"A\"}", &ctx, &args, true).text,
            "x%20y"
        );
        assert!(uses_selection("{selection} hello"));
        assert!(uses_selection("x{selectedText}"));
        assert!(!uses_selection("{clipboard}"));
        assert_eq!(
            expand("{selectedText}", &ctx, &Default::default()).text,
            expand("{selection}", &ctx, &Default::default()).text
        );
        assert_eq!(
            expand("{SelectedText}", &ctx, &Default::default()).text,
            "a b&c"
        );
        assert_eq!(
            expand("{selectedText | trim | uppercase}", &ctx, &Default::default()).text,
            "A B&C"
        );
        assert_eq!(
            expand_with("{selectedText}", &ctx, &Default::default(), true).text,
            "a%20b%26c"
        );
        assert_eq!(
            expand("{selectedText offset=1}", &ctx, &Default::default()).text,
            "{selectedText offset=1}"
        );
        assert!(uses_selection("a {selection} b"));
        assert!(uses_selection("a {selectedText} b"));
        assert!(!uses_selection("{clipboard} {date}"));
        assert!(!uses_selection("{selection offset=1}"));
    }

    #[test]
    fn empty_clipboard_history_expands_to_nothing() {
        let ctx = ExpandContext {
            clipboard: vec![],
            selection: Some("".into()),
            now: unix(2026, 7, 24, 13, 5, 0),
            locale: "en_US_POSIX".into(),
            tz: "UTC".into(),
            uuid: || "u".into(),
        };
        assert_eq!(expand("[{clipboard}]", &ctx, &Default::default()).text, "[]");
    }

    #[test]
    fn destination_mode_leaves_cursor_and_snippet_refs_verbatim() {
        let ctx = ExpandContext {
            clipboard: vec!["hi".into()],
            selection: None,
            now: 0,
            locale: "en".into(),
            tz: "UTC".into(),
            uuid: || "u".into(),
        };
        let stripped = expand("A{cursor}B{snippet:X}", &ctx, &Default::default());
        assert_eq!(stripped.text, "AB{snippet:X}");
        assert_eq!(stripped.cursor, Some(1));
        let kept = expand_destination_template(
            "A{cursor}B{snippet:X}",
            &ctx,
            &Default::default(),
            false,
        );
        assert_eq!(kept.text, "A{cursor}B{snippet:X}");
        assert!(kept.cursor.is_none());
    }
}
