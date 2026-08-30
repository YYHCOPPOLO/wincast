//! Locale-independent number formatting, matching v0.10.2 `CalcFormatter`.

const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;

/// Human-facing: ≤10 significant digits, trailing zeros trimmed, thousands separators.
pub(crate) fn display(value: f64) -> String {
    grouped(&copy_text(value))
}

/// Same rounding, no grouping — what lands on the pasteboard.
pub(crate) fn copy_text(value: f64) -> String {
    let v = if value == 0.0 { 0.0 } else { value };
    if v.round() == v && v.abs() <= MAX_EXACT_INTEGER {
        format!("{v:.0}")
    } else {
        format_g(v, 10)
    }
}

/// Money: 2 decimals, widening below a cent. Never `%g`.
pub(crate) fn currency(value: f64) -> String {
    let magnitude = value.abs();
    if magnitude < 1e-9 {
        return "0.00".into();
    }
    if magnitude >= 0.01 {
        return format!("{value:.2}");
    }
    let digits = 3 - magnitude.log10().floor() as i32;
    let mut text = format!("{value:.prec$}", prec = digits as usize);
    while text.ends_with('0') {
        text.pop();
    }
    text
}

/// Whole feet + remaining inches, for the bare metric-length auto-conversion only.
pub(crate) fn compound_feet_inches(feet: f64) -> String {
    let sign = if feet < 0.0 { "-" } else { "" };
    let magnitude = feet.abs();
    let whole_feet = magnitude.trunc();
    let inches = (magnitude - whole_feet) * 12.0;
    let feet_part = if whole_feet == 0.0 {
        String::new()
    } else {
        let unit = if whole_feet == 1.0 { "foot" } else { "feet" };
        format!("{sign}{} {unit}", display(whole_feet))
    };
    let inch_text = display(inches);
    let inch_unit = if inch_text == "1" { "inch" } else { "inches" };
    let inch_part = format!("{inch_text} {inch_unit}");
    if feet_part.is_empty() {
        format!("{sign}{inch_part}")
    } else {
        format!("{feet_part} {inch_part}")
    }
}

/// Insert `,` every three integer digits. Exponent-form strings pass through untouched.
pub(crate) fn grouped(text: &str) -> String {
    if text.contains('e') || text.contains('E') {
        return text.to_string();
    }
    let (sign, unsigned) = if let Some(rest) = text.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", text)
    };
    let (int_digits, fraction) = match unsigned.split_once('.') {
        Some((int, frac)) => (int, format!(".{frac}")),
        None => (unsigned, String::new()),
    };
    if int_digits.len() <= 3 {
        return text.to_string();
    }
    let mut grouped_int = String::new();
    for (i, ch) in int_digits.chars().enumerate() {
        if i > 0 && (int_digits.len() - i) % 3 == 0 {
            grouped_int.push(',');
        }
        grouped_int.push(ch);
    }
    format!("{sign}{grouped_int}{fraction}")
}

/// C `%.Pg` with P significant digits: fixed unless the exponent is `< -4` or `>= P`.
fn format_g(value: f64, precision: usize) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let abs = value.abs();
    if abs == 0.0 {
        return format!("{sign}0");
    }

    let mut exp = abs.log10().floor() as i32;
    // Correct log10 rounding around powers of ten.
    if abs >= pow10(exp + 1) {
        exp += 1;
    } else if abs < pow10(exp) {
        exp -= 1;
    }

    let scale_exp = precision as i32 - 1 - exp;
    let mut digits = (abs * pow10(scale_exp)).round() as u128;
    let overflow_at = 10u128.pow(precision as u32);
    if digits >= overflow_at {
        digits /= 10;
        exp += 1;
    }
    let mut digit_str = digits.to_string();
    while digit_str.len() < precision {
        digit_str.insert(0, '0');
    }
    if digit_str.len() > precision {
        digit_str.truncate(precision);
    }

    if exp < -4 || exp >= precision as i32 {
        let (head, rest) = digit_str.split_at(1);
        let rest = rest.trim_end_matches('0');
        let mantissa = if rest.is_empty() {
            head.to_string()
        } else {
            format!("{head}.{rest}")
        };
        format!("{sign}{mantissa}e{exp:+03}")
    } else if exp < 0 {
        let mut out = String::from("0.");
        for _ in 0..(-exp - 1) {
            out.push('0');
        }
        out.push_str(digit_str.trim_end_matches('0'));
        format!("{sign}{out}")
    } else {
        let int_len = exp as usize + 1;
        let mut out = String::new();
        if int_len >= digit_str.len() {
            out.push_str(&digit_str);
            out.extend(std::iter::repeat('0').take(int_len - digit_str.len()));
        } else {
            out.push_str(&digit_str[..int_len]);
            let frac = digit_str[int_len..].trim_end_matches('0');
            if !frac.is_empty() {
                out.push('.');
                out.push_str(frac);
            }
        }
        format!("{sign}{out}")
    }
}

fn pow10(exp: i32) -> f64 {
    if exp >= 0 {
        10f64.powi(exp)
    } else {
        1.0 / 10f64.powi(-exp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_text_matches_oracle() {
        assert_eq!(copy_text(4.0), "4");
        assert_eq!(copy_text(1.0 / 3.0), "0.3333333333");
        assert_eq!(copy_text(1e-5), "1e-05");
        assert_eq!(display(10_000.0), "10,000");
        assert_eq!(copy_text(10.0 * 1000.0 / 1609.344), "6.213711922");
        assert_eq!(copy_text(std::f64::consts::PI * 2.0), "6.283185307");
        assert_eq!(grouped("15700.00"), "15,700.00");
        assert_eq!(currency(14.2), "14.20");
        assert_eq!(currency(1.0 / 18053.0), "0.00005539");
    }
}
