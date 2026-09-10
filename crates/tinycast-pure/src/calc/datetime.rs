//! Natural-language date/time. Four grammars; see calculator.md. UTC from injected unix `now`.

use super::format;
use super::CalcResult;

enum MomentBias {
    Future,
    Past,
}

struct Moment {
    unix: i64,
    has_time: bool,
}

pub(crate) fn evaluate(raw: &str, now: i64) -> Option<CalcResult> {
    let echo = raw.trim();
    let query = echo.to_lowercase();
    if query.is_empty() {
        return None;
    }

    let has_until =
        query.contains(" till ") || query.contains(" until ") || query.contains(" til ");
    let has_since = query.contains(" since ");
    let has_arith = query.contains(" + ") || query.contains(" - ");
    if !has_until && !has_since && !has_arith {
        return None;
    }

    if has_until {
        if let Some(result) = parse_until(&query, echo, now) {
            return Some(result);
        }
    }
    if has_since {
        if let Some(result) = parse_since(&query, echo, now) {
            return Some(result);
        }
    }
    if has_arith {
        if let Some(result) = parse_arithmetic(&query, echo, now) {
            return Some(result);
        }
    }
    None
}

fn parse_until(query: &str, echo: &str, now: i64) -> Option<CalcResult> {
    let connector = [" until ", " till ", " til "]
        .into_iter()
        .find(|c| query.contains(c))?;
    let (left, right) = split2(query, connector)?;
    let unit = duration_unit(left)?;
    let moment = parse_moment(right, now, MomentBias::Future)?;

    let reference = if unit.sub_day { now } else { start_of_day(now) };
    let target = if unit.sub_day {
        moment.unix
    } else {
        start_of_day(moment.unix)
    };
    let value = duration_value(&unit, reference, target);
    duration_card(echo, value, &unit, reference, target, now, true)
}

fn parse_since(query: &str, echo: &str, now: i64) -> Option<CalcResult> {
    let (left, right) = split2(query, " since ")?;
    let unit = duration_unit(left)?;
    let moment = parse_moment(right, now, MomentBias::Past)?;

    let reference = if unit.sub_day { now } else { start_of_day(now) };
    let past = if unit.sub_day {
        moment.unix
    } else {
        start_of_day(moment.unix)
    };
    let value = duration_value(&unit, past, reference);
    duration_card(echo, value, &unit, past, reference, now, true)
}

fn parse_arithmetic(query: &str, echo: &str, now: i64) -> Option<CalcResult> {
    let plus = query.find(" + ");
    let minus = query.find(" - ");
    let (op_at, op, sep_len) = match (plus, minus) {
        (Some(p), Some(m)) if p < m => (p, '+', 3),
        (Some(_), Some(m)) => (m, '-', 3),
        (Some(p), None) => (p, '+', 3),
        (None, Some(m)) => (m, '-', 3),
        _ => return None,
    };
    let left = &query[..op_at];
    let right = &query[op_at + sep_len..];
    let base = parse_moment(left, now, MomentBias::Future)?;

    if let Some(duration) = parse_duration_phrase(right) {
        if op == '-' && duration.count == i32::MIN {
            return None;
        }
        let signed = if op == '-' {
            -duration.count
        } else {
            duration.count
        };
        let result = base
            .unix
            .checked_add(i64::from(signed) * duration.seconds)?;
        let has_time = base.has_time || duration.sub_day;
        let display = moment_string(result, has_time, now);
        let source_badge = moment_string(base.unix, base.has_time, now);
        return Some(CalcResult::value(
            echo,
            display.clone(),
            display,
            Some(source_badge),
            Some("Result".into()),
        ));
    }

    if op != '-' {
        return None;
    }
    if !left.chars().any(char::is_alphabetic) && !right.chars().any(char::is_alphabetic) {
        return None;
    }
    let other = parse_moment(right, now, MomentBias::Future)?;
    let days = (start_of_day(base.unix) - start_of_day(other.unix)) / 86400;
    let word = if days.abs() == 1 { "day" } else { "days" };
    let display = format!("{days} {word}");
    Some(CalcResult::value(
        echo,
        display.clone(),
        display,
        Some(date_string(base.unix, now)),
        Some(date_string(other.unix, now)),
    ))
}

fn duration_card(
    echo: &str,
    value: f64,
    unit: &DurUnit,
    source_unix: i64,
    target_unix: i64,
    now: i64,
    _from_until_or_since: bool,
) -> Option<CalcResult> {
    let word = if value == 1.0 {
        unit.singular
    } else {
        unit.plural
    };
    let source = if unit.sub_day {
        time_string(source_unix)
    } else {
        date_string(source_unix, now)
    };
    let target = if unit.sub_day {
        time_string(target_unix)
    } else {
        date_string(target_unix, now)
    };
    let display = format!("{} {word}", format::display(value));
    let copy = format!("{} {word}", format::copy_text(value));
    Some(CalcResult::value(
        echo,
        display,
        copy,
        Some(source),
        Some(target),
    ))
}

fn duration_value(unit: &DurUnit, from: i64, to: i64) -> f64 {
    match unit.kind {
        DurKind::Day => ((to - from) / 86400) as f64,
        DurKind::Week => ((to - from) / 86400) as f64 / 7.0,
        DurKind::SubSecond => (to - from) as f64 / unit.seconds,
    }
}

fn parse_moment(phrase: &str, now: i64, bias: MomentBias) -> Option<Moment> {
    let atoms = atomize(phrase);
    match atoms.len() {
        1 => parse_single(&atoms[0], now, bias),
        2 => parse_pair(&atoms[0], &atoms[1], now, bias),
        _ => None,
    }
}

fn parse_single(atom: &str, now: i64, bias: MomentBias) -> Option<Moment> {
    let sod = start_of_day(now);
    match atom {
        "now" => {
            return Some(Moment {
                unix: now,
                has_time: true,
            })
        }
        "today" => {
            return Some(Moment {
                unix: sod,
                has_time: false,
            })
        }
        "tomorrow" => {
            return Some(Moment {
                unix: sod + 86400,
                has_time: false,
            })
        }
        "yesterday" => {
            return Some(Moment {
                unix: sod - 86400,
                has_time: false,
            })
        }
        "noon" => return clock_moment(12, 0, now, bias),
        "midnight" => return clock_moment(0, 0, now, bias),
        _ => {}
    }
    if let Some(weekday) = weekday_by_name(atom) {
        return next_weekday(weekday, false, matches!(bias, MomentBias::Past), now);
    }
    if let Some(month) = month_by_name(atom) {
        return month_day_moment(month, 1, now, bias);
    }
    parse_date_atom(atom, now, bias)
}

fn parse_pair(a: &str, b: &str, now: i64, bias: MomentBias) -> Option<Moment> {
    if let (Some(month), Ok(day)) = (month_by_name(b), a.parse::<u32>()) {
        return month_day_moment(month, day, now, bias);
    }
    if let (Some(month), Ok(day)) = (month_by_name(a), b.parse::<u32>()) {
        return month_day_moment(month, day, now, bias);
    }
    if b == "am" || b == "pm" {
        if let Some((hour, minute)) = parse_clock(a) {
            if !(1..=12).contains(&hour) {
                return None;
            }
            let adjusted = if b == "pm" {
                (hour % 12) + 12
            } else {
                hour % 12
            };
            return clock_moment(adjusted, minute, now, bias);
        }
    }
    if a == "next" || a == "last" {
        if let Some(weekday) = weekday_by_name(b) {
            return next_weekday(weekday, a == "next", a == "last", now);
        }
        if let Some(month) = month_by_name(b) {
            let pair_bias = if a == "last" {
                MomentBias::Past
            } else {
                MomentBias::Future
            };
            return month_day_moment(month, 1, now, pair_bias);
        }
    }
    None
}

fn parse_date_atom(atom: &str, now: i64, bias: MomentBias) -> Option<Moment> {
    if atom.contains(':') {
        let (hour, minute) = parse_clock(atom)?;
        return clock_moment(hour, minute, now, bias);
    }
    if atom.contains('-') {
        let parts: Vec<&str> = atom.split('-').collect();
        if parts.len() == 3 {
            let year: i32 = parts[0].parse().ok()?;
            if year <= 31 {
                return None;
            }
            let month: u32 = parts[1].parse().ok()?;
            let day: u32 = parts[2].parse().ok()?;
            let unix = make_date(year, month, day)?;
            return Some(Moment {
                unix,
                has_time: false,
            });
        }
        return None;
    }
    if atom.contains('/') {
        let parts: Vec<&str> = atom.split('/').collect();
        if parts.len() == 2 {
            let month: u32 = parts[0].parse().ok()?;
            let day: u32 = parts[1].parse().ok()?;
            return month_day_moment(month, day, now, bias);
        }
        if parts.len() == 3 {
            let month: u32 = parts[0].parse().ok()?;
            let day: u32 = parts[1].parse().ok()?;
            let year: i32 = parts[2].parse().ok()?;
            let unix = make_date(full_year(year), month, day)?;
            return Some(Moment {
                unix,
                has_time: false,
            });
        }
    }
    None
}

fn clock_moment(hour: u32, minute: u32, now: i64, bias: MomentBias) -> Option<Moment> {
    if hour > 23 || minute > 59 {
        return None;
    }
    let sod = start_of_day(now);
    let mut date = sod + i64::from(hour) * 3600 + i64::from(minute) * 60;
    match bias {
        MomentBias::Future if date <= now => date += 86400,
        MomentBias::Past if date > now => date -= 86400,
        _ => {}
    }
    Some(Moment {
        unix: date,
        has_time: true,
    })
}

fn month_day_moment(month: u32, day: u32, now: i64, bias: MomentBias) -> Option<Moment> {
    let (year, _, _) = civil_from_days(now.div_euclid(86400));
    let this_year = make_date(year, month, day)?;
    let sod = start_of_day(now);
    match bias {
        MomentBias::Future => {
            if this_year >= sod {
                Some(Moment {
                    unix: this_year,
                    has_time: false,
                })
            } else {
                Some(Moment {
                    unix: make_date(year + 1, month, day)?,
                    has_time: false,
                })
            }
        }
        MomentBias::Past => {
            if this_year <= sod {
                Some(Moment {
                    unix: this_year,
                    has_time: false,
                })
            } else {
                Some(Moment {
                    unix: make_date(year - 1, month, day)?,
                    has_time: false,
                })
            }
        }
    }
}

fn next_weekday(weekday: u32, offset_to_future: bool, past: bool, now: i64) -> Option<Moment> {
    let sod = start_of_day(now);
    let today = weekday_of(sod);
    if past {
        let mut back = (today + 7 - weekday) % 7;
        if back == 0 {
            back = 7;
        }
        return Some(Moment {
            unix: sod - i64::from(back) * 86400,
            has_time: false,
        });
    }
    let mut ahead = (weekday + 7 - today) % 7;
    if ahead == 0 && offset_to_future {
        ahead = 7;
    }
    Some(Moment {
        unix: sod + i64::from(ahead) * 86400,
        has_time: false,
    })
}

enum DurKind {
    SubSecond,
    Day,
    Week,
}

struct DurUnit {
    seconds: f64,
    singular: &'static str,
    plural: &'static str,
    kind: DurKind,
    sub_day: bool,
}

fn duration_unit(phrase: &str) -> Option<DurUnit> {
    let last = phrase.split(' ').last()?;
    Some(match last {
        "s" | "sec" | "secs" | "second" | "seconds" => DurUnit {
            seconds: 1.0,
            singular: "second",
            plural: "seconds",
            kind: DurKind::SubSecond,
            sub_day: true,
        },
        "min" | "mins" | "minute" | "minutes" => DurUnit {
            seconds: 60.0,
            singular: "minute",
            plural: "minutes",
            kind: DurKind::SubSecond,
            sub_day: true,
        },
        "h" | "hr" | "hrs" | "hour" | "hours" => DurUnit {
            seconds: 3600.0,
            singular: "hour",
            plural: "hours",
            kind: DurKind::SubSecond,
            sub_day: true,
        },
        "d" | "day" | "days" => DurUnit {
            seconds: 86400.0,
            singular: "day",
            plural: "days",
            kind: DurKind::Day,
            sub_day: false,
        },
        "wk" | "week" | "weeks" => DurUnit {
            seconds: 604800.0,
            singular: "week",
            plural: "weeks",
            kind: DurKind::Week,
            sub_day: false,
        },
        _ => return None,
    })
}

struct DurationPhrase {
    count: i32,
    seconds: i64,
    sub_day: bool,
}

fn parse_duration_phrase(phrase: &str) -> Option<DurationPhrase> {
    let atoms = atomize(phrase);
    if atoms.len() != 2 {
        return None;
    }
    let count: i32 = atoms[0].parse().ok()?;
    match atoms[1].as_str() {
        "s" | "sec" | "secs" | "second" | "seconds" => Some(DurationPhrase {
            count,
            seconds: 1,
            sub_day: true,
        }),
        "min" | "mins" | "minute" | "minutes" => Some(DurationPhrase {
            count,
            seconds: 60,
            sub_day: true,
        }),
        "h" | "hr" | "hrs" | "hour" | "hours" => Some(DurationPhrase {
            count,
            seconds: 3600,
            sub_day: true,
        }),
        "d" | "day" | "days" => Some(DurationPhrase {
            count,
            seconds: 86400,
            sub_day: false,
        }),
        "wk" | "week" | "weeks" => {
            let days = count.checked_mul(7)?;
            Some(DurationPhrase {
                count: days,
                seconds: 86400,
                sub_day: false,
            })
        }
        _ => None,
    }
}

fn moment_string(unix: i64, has_time: bool, now: i64) -> String {
    let day = date_string(unix, now);
    if has_time {
        format!("{day} at {}", time_string(unix))
    } else {
        day
    }
}

fn date_string(unix: i64, now: i64) -> String {
    let (year, month, day) = civil_from_days(unix.div_euclid(86400));
    let (now_year, _, _) = civil_from_days(now.div_euclid(86400));
    let weekday = WEEKDAYS[(weekday_of(unix) as usize + 6) % 7];
    let month_name = MONTHS[month as usize - 1];
    if year == now_year {
        format!("{weekday}, {day} {month_name}")
    } else {
        format!("{weekday}, {day} {month_name}, {year}")
    }
}

fn time_string(unix: i64) -> String {
    let sod = unix.rem_euclid(86400) as u32;
    let hour = sod / 3600;
    let minute = (sod % 3600) / 60;
    let ampm = if hour < 12 { "AM" } else { "PM" };
    let h12 = match hour % 12 {
        0 => 12,
        h => h,
    };
    format!("{h12}:{minute:02} {ampm}")
}

fn atomize(text: &str) -> Vec<String> {
    let mut atoms = Vec::new();
    let mut current = String::new();
    let mut current_is_number = false;
    let flush = |atoms: &mut Vec<String>, current: &mut String| {
        if !current.is_empty() {
            atoms.push(std::mem::take(current));
        }
    };
    for ch in text.chars() {
        if ch == ' ' {
            flush(&mut atoms, &mut current);
            continue;
        }
        let is_numeric = ch.is_ascii_digit() || matches!(ch, ':' | '/' | '-' | '.');
        let is_letter = ch.is_alphabetic();
        if current.is_empty() {
            current.push(ch);
            current_is_number = is_numeric && !is_letter;
        } else if is_letter && current_is_number {
            flush(&mut atoms, &mut current);
            current.push(ch);
            current_is_number = false;
        } else if is_numeric && !is_letter && !current_is_number {
            flush(&mut atoms, &mut current);
            current.push(ch);
            current_is_number = true;
        } else {
            current.push(ch);
        }
    }
    flush(&mut atoms, &mut current);
    atoms
}

fn parse_clock(atom: &str) -> Option<(u32, u32)> {
    if atom.contains(':') {
        let mut parts = atom.split(':');
        let hour = parts.next()?.parse().ok()?;
        let minute = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some((hour, minute))
    } else {
        Some((atom.parse().ok()?, 0))
    }
}

fn make_date(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86400)
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

fn start_of_day(unix: i64) -> i64 {
    unix.div_euclid(86400) * 86400
}

fn weekday_of(unix: i64) -> u32 {
    // Sunday = 1 … Saturday = 7. Unix day 0 is Thursday = 5.
    ((unix.div_euclid(86400) + 4).rem_euclid(7) + 1) as u32
}

fn full_year(year: i32) -> i32 {
    if year >= 100 {
        year
    } else if year <= 68 {
        2000 + year
    } else {
        1900 + year
    }
}

fn split2<'a>(query: &'a str, sep: &str) -> Option<(&'a str, &'a str)> {
    let mut parts = query.splitn(3, sep);
    let a = parts.next()?;
    let b = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((a, b))
}

fn month_by_name(name: &str) -> Option<u32> {
    Some(match name {
        "january" | "jan" => 1,
        "february" | "feb" => 2,
        "march" | "mar" => 3,
        "april" | "apr" => 4,
        "may" => 5,
        "june" | "jun" => 6,
        "july" | "jul" => 7,
        "august" | "aug" => 8,
        "september" | "sep" | "sept" => 9,
        "october" | "oct" => 10,
        "november" | "nov" => 11,
        "december" | "dec" => 12,
        _ => return None,
    })
}

fn weekday_by_name(name: &str) -> Option<u32> {
    Some(match name {
        "sunday" | "sun" => 1,
        "monday" | "mon" => 2,
        "tuesday" | "tue" | "tues" => 3,
        "wednesday" | "wed" => 4,
        "thursday" | "thu" | "thurs" => 5,
        "friday" | "fri" => 6,
        "saturday" | "sat" => 7,
        _ => return None,
    })
}

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const MONTHS: [&str; 12] = [
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

/// Days since Unix epoch for a Gregorian civil date (Howard Hinnant).
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
