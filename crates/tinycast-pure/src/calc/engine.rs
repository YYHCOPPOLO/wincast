//! Evaluate pipeline: datetime → tokenize → partial → bases → units → quantity → FX → arithmetic.

use super::currency::{self, CurrencyDef, CurrencyRates};
use super::datetime;
use super::format;
use super::units::{self, UnitCategory, UnitDef};
use super::CalcResult;

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    CompactNumber(f64),
    IntLiteral { value: u64, radix: u32 },
    Ident(String),
    Op(char),
    Arrow,
}

pub fn evaluate(
    query: &str,
    now: i64,
    rates: Option<&CurrencyRates>,
    region: Option<&str>,
) -> Option<CalcResult> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 256 {
        return None;
    }

    if let Some(date_time) = datetime::evaluate(query, now) {
        return Some(date_time);
    }

    let tokens = tokenize(query)?;
    if tokens.is_empty() {
        return None;
    }

    if let Some(partial) = partial_result(&tokens, query, now, rates, region) {
        return Some(partial);
    }

    if tokens.len() == 1 {
        match &tokens[0] {
            Token::IntLiteral { value, radix } if *radix != 10 => {
                let display = format::grouped(&value.to_string());
                return Some(CalcResult::value(
                    query,
                    display,
                    value.to_string(),
                    Some(base_name(*radix).into()),
                    Some("Decimal".into()),
                ));
            }
            Token::CompactNumber(value) => {
                return Some(CalcResult::value(
                    query,
                    format::display(*value),
                    format::copy_text(*value),
                    Some("Expression".into()),
                    Some("Result".into()),
                ));
            }
            _ => return None,
        }
    }

    if let Some(base) = base_conversion(&tokens, query) {
        return Some(base);
    }

    if let Some(conversion) = parse_unit_conversion(&tokens).or_else(|| parse_unit_pair(&tokens)) {
        return Some(unit_conversion_result(query, conversion));
    }

    if let Some(quantity) = eval_quantity(&tokens, query, rates, region, false) {
        return Some(quantity);
    }

    if let Some(conversion) = parse_currency_conversion(&tokens, rates) {
        return Some(currency_conversion_result(query, conversion));
    }

    if let Some(bare) = parse_bare_conversion(&tokens) {
        let (display, copy_text) = if bare.compound {
            let text = format::compound_feet_inches(bare.output);
            (text.clone(), text)
        } else {
            (
                format!("{} {}", format::display(bare.output), bare.to.symbol),
                format!("{} {}", format::copy_text(bare.output), bare.to.symbol),
            )
        };
        return Some(CalcResult::value(
            format!("{} {}", format::display(bare.input), bare.from.symbol),
            display,
            copy_text,
            Some(bare.from.name.into()),
            Some(bare.to.name.into()),
        ));
    }

    if let Some(percent) = eval_percent(&tokens, query) {
        return Some(percent);
    }

    let has_digit = query.chars().any(|c| c.is_ascii_digit());
    let has_e = query.to_lowercase().contains('e');
    if !has_digit && !has_e && !query.contains('π') {
        return None;
    }

    let value = parser_eval(&tokens)?;
    Some(CalcResult::value(
        pretty_expression(query),
        format::display(value),
        format::copy_text(value),
        Some("Expression".into()),
        Some("Result".into()),
    ))
}

fn partial_result(
    tokens: &[Token],
    query: &str,
    now: i64,
    rates: Option<&CurrencyRates>,
    region: Option<&str>,
) -> Option<CalcResult> {
    let operator_text = partial_operator_text(tokens.last()?)?;
    let prefix = &tokens[..tokens.len() - 1];
    if prefix.is_empty() {
        return None;
    }

    if let Some(quantity) = eval_quantity(prefix, &token_query(prefix), rates, region, true) {
        let expression = format!("{} {operator_text}", quantity.expression);
        return Some(quantity.with_expression(expression));
    }

    if let Some(complete) = evaluate(&token_query(prefix), now, rates, region) {
        return Some(complete.with_expression(pretty_expression(query)));
    }

    let value = parser_eval(prefix)?;
    Some(CalcResult::value(
        pretty_expression(query),
        format::display(value),
        format::copy_text(value),
        Some("Expression".into()),
        Some("Result".into()),
    ))
}

fn partial_operator_text(token: &Token) -> Option<String> {
    match token {
        Token::Op('*') => Some("×".into()),
        Token::Op('/') => Some("÷".into()),
        Token::Op(op @ ('+' | '-' | '^')) => Some(op.to_string()),
        _ => None,
    }
}

fn token_query(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|token| match token {
            Token::Number(value) | Token::CompactNumber(value) => format::copy_text(*value),
            Token::IntLiteral { value, radix } => {
                let prefix = match radix {
                    16 => "0x",
                    2 => "0b",
                    8 => "0o",
                    _ => "",
                };
                format!("{prefix}{}", format_int_radix(*value, *radix))
            }
            Token::Ident(name) => name.clone(),
            Token::Op(op) => op.to_string(),
            Token::Arrow => "->".into(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_int_radix(value: u64, radix: u32) -> String {
    match radix {
        16 => format!("{value:x}"),
        2 => format!("{value:b}"),
        8 => format!("{value:o}"),
        _ => value.to_string(),
    }
}

fn pretty_expression(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('*', "×")
        .replace('/', "÷")
}

fn base_conversion(tokens: &[Token], query: &str) -> Option<CalcResult> {
    let n = tokens.len();
    if n < 3 || !is_connector(&tokens[n - 2]) {
        return None;
    }
    let Token::Ident(target) = &tokens[n - 1] else {
        return None;
    };
    let value_tokens = &tokens[..n - 2];
    let literal_text = query.split_whitespace().next().unwrap_or(query);
    let (source, source_badge, source_text) = if value_tokens.len() == 1 {
        match &value_tokens[0] {
            Token::IntLiteral { value, radix } => {
                (*value, base_name(*radix), literal_text.to_string())
            }
            other if decimal_literal(other).is_some() => {
                let value = decimal_literal(other).unwrap();
                if value >= 0.0 && value.round() == value && value <= 9_007_199_254_740_992.0 {
                    (
                        value as u64,
                        "Decimal",
                        literal_text.to_string(),
                    )
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    } else if let Some(value) = parser_eval(value_tokens) {
        if value >= 0.0 && value.round() == value && value <= 9_007_199_254_740_992.0 {
            let source = value as u64;
            (
                source,
                "Decimal",
                format::grouped(&source.to_string()),
            )
        } else {
            return None;
        }
    } else {
        return None;
    };

    let (output, target_badge) = match target.as_str() {
        "hex" | "hexadecimal" => (format!("0x{source:X}"), "Hexadecimal"),
        "binary" | "bin" => (format!("0b{source:b}"), "Binary"),
        "octal" | "oct" => (format!("0o{source:o}"), "Octal"),
        "decimal" | "dec" => (format::grouped(&source.to_string()), "Decimal"),
        _ => return None,
    };
    let copy = output.replace(',', "");
    Some(CalcResult::value(
        source_text,
        output,
        copy,
        Some(source_badge.into()),
        Some(target_badge.into()),
    ))
}

fn base_name(radix: u32) -> &'static str {
    match radix {
        16 => "Hexadecimal",
        2 => "Binary",
        8 => "Octal",
        _ => "Decimal",
    }
}

fn decimal_literal(token: &Token) -> Option<f64> {
    match token {
        Token::Number(v) | Token::CompactNumber(v) => Some(*v),
        _ => None,
    }
}

fn is_connector(token: &Token) -> bool {
    match token {
        Token::Arrow => true,
        Token::Ident(name) if name == "to" || name == "in" => true,
        _ => false,
    }
}

enum UnitConv {
    Value {
        input: f64,
        from: UnitDef,
        to: UnitDef,
        output: f64,
    },
    Mismatch {
        from: UnitDef,
        to: UnitDef,
    },
}

fn parse_unit_conversion(tokens: &[Token]) -> Option<UnitConv> {
    let n = tokens.len();
    if n < 3 || !is_connector(&tokens[n - 2]) {
        return None;
    }
    let Token::Ident(to_name) = &tokens[n - 1] else {
        return None;
    };
    let Token::Ident(from_name) = &tokens[n - 3] else {
        return None;
    };
    let to = units::lookup(to_name)?;
    let from = units::lookup(from_name)?;
    let value_tokens = &tokens[..n - 3];
    let input = if value_tokens.is_empty() {
        1.0
    } else {
        parser_eval(value_tokens)?
    };
    if from.category != to.category {
        return Some(UnitConv::Mismatch { from, to });
    }
    let output = units::convert(input, from, to);
    output.is_finite().then_some(UnitConv::Value {
        input,
        from,
        to,
        output,
    })
}

fn parse_unit_pair(tokens: &[Token]) -> Option<UnitConv> {
    if tokens.len() != 2 {
        return None;
    }
    let Token::Ident(from_name) = &tokens[0] else {
        return None;
    };
    let Token::Ident(to_name) = &tokens[1] else {
        return None;
    };
    let from = units::lookup(from_name)?;
    let to = units::lookup(to_name)?;
    if from.category != to.category {
        return None;
    }
    let output = units::convert(1.0, from, to);
    output.is_finite().then_some(UnitConv::Value {
        input: 1.0,
        from,
        to,
        output,
    })
}

fn unit_conversion_result(query: &str, conversion: UnitConv) -> CalcResult {
    match conversion {
        UnitConv::Value {
            input,
            from,
            to,
            output,
        } => CalcResult::value(
            format!("{} {}", format::display(input), from.symbol),
            format!("{} {}", format::display(output), to.symbol),
            format!("{} {}", format::copy_text(output), to.symbol),
            Some(from.name.into()),
            Some(to.name.into()),
        ),
        UnitConv::Mismatch { from, to } => CalcResult::error(
            query,
            format!(
                "Cannot convert {} to {}.",
                from.category.display_name(),
                to.category.display_name()
            ),
        ),
    }
}

struct BareConversion {
    input: f64,
    from: UnitDef,
    to: UnitDef,
    output: f64,
    compound: bool,
}

fn parse_bare_conversion(tokens: &[Token]) -> Option<BareConversion> {
    let Token::Ident(from_name) = tokens.last()? else {
        return None;
    };
    if matches!(from_name.as_str(), "c" | "f" | "k") {
        return None;
    }
    let from = units::lookup(from_name)?;
    let (to_name, compound) = units::auto_target(from.symbol)?;
    let to = units::lookup(to_name)?;
    let input = parser_eval(&tokens[..tokens.len() - 1])?;
    let output = units::convert(input, from, to);
    output.is_finite().then_some(BareConversion {
        input,
        from,
        to,
        output,
        compound,
    })
}

enum CurrencyConv {
    Value {
        input: f64,
        from: CurrencyDef,
        to: CurrencyDef,
        output: f64,
    },
    Mismatch {
        from: String,
        to: String,
    },
    NoRate {
        code: String,
    },
    Unavailable,
}

fn parse_currency_conversion(tokens: &[Token], rates: Option<&CurrencyRates>) -> Option<CurrencyConv> {
    let tokens = amount_first(tokens);
    let n = tokens.len();
    if n < 3 || !is_connector(&tokens[n - 2]) {
        return None;
    }
    let Token::Ident(to_name) = &tokens[n - 1] else {
        return None;
    };
    let Token::Ident(from_name) = &tokens[n - 3] else {
        return None;
    };
    match (currency::lookup(from_name), currency::lookup(to_name)) {
        (None, None) => None,
        (Some(_), None) => {
            let to = units::lookup(to_name)?;
            Some(CurrencyConv::Mismatch {
                from: currency::CATEGORY_NAME.into(),
                to: to.category.display_name().into(),
            })
        }
        (None, Some(_)) => {
            let from = units::lookup(from_name)?;
            Some(CurrencyConv::Mismatch {
                from: from.category.display_name().into(),
                to: currency::CATEGORY_NAME.into(),
            })
        }
        (Some(from), Some(to)) => {
            let value_tokens = &tokens[..n - 3];
            let input = if value_tokens.is_empty() {
                1.0
            } else {
                parser_eval(value_tokens)?
            };
            let Some(rates) = rates else {
                return Some(CurrencyConv::Unavailable);
            };
            if rates.rate(from.code).is_none() {
                return Some(CurrencyConv::NoRate {
                    code: from.code.into(),
                });
            }
            if rates.rate(to.code).is_none() {
                return Some(CurrencyConv::NoRate {
                    code: to.code.into(),
                });
            }
            match rates.convert(input, from.code, to.code) {
                Some(output) => Some(CurrencyConv::Value {
                    input,
                    from,
                    to,
                    output,
                }),
                None => Some(CurrencyConv::NoRate {
                    code: to.code.into(),
                }),
            }
        }
    }
}

fn amount_first(tokens: &[Token]) -> Vec<Token> {
    if tokens.len() >= 2 {
        if let Token::Ident(name) = &tokens[0] {
            if currency::lookup(name).is_some() && number_token(&tokens[1]) {
                let mut reordered = tokens.to_vec();
                reordered.swap(0, 1);
                return reordered;
            }
        }
    }
    tokens.to_vec()
}

fn number_token(token: &Token) -> bool {
    matches!(token, Token::Number(_) | Token::CompactNumber(_))
}

fn currency_conversion_result(query: &str, conversion: CurrencyConv) -> CalcResult {
    match conversion {
        CurrencyConv::Value {
            input,
            from,
            to,
            output,
        } => {
            let amount = format::currency(output);
            CalcResult::value(
                format!("{} {}", format::display(input), from.code),
                format!("{} {}", format::grouped(&amount), to.code),
                format!("{amount} {}", to.code),
                Some(from.name.into()),
                Some(to.name.into()),
            )
        }
        CurrencyConv::Mismatch { from, to } => {
            CalcResult::error(query, format!("Cannot convert {from} to {to}."))
        }
        CurrencyConv::NoRate { code } => {
            CalcResult::error(query, format!("No exchange rate for {code}."))
        }
        CurrencyConv::Unavailable => CalcResult::error(
            query,
            "Exchange rates unavailable — check your connection.",
        ),
    }
}

fn eval_percent(tokens: &[Token], query: &str) -> Option<CalcResult> {
    parse_off(tokens, query).or_else(|| parse_as_percent_of(tokens, query))
}

fn parse_off(tokens: &[Token], query: &str) -> Option<CalcResult> {
    let off = tokens.iter().position(|t| matches!(t, Token::Ident(n) if n == "off"))?;
    if off < 2 || tokens[off - 1] != Token::Op('%') {
        return None;
    }
    let pct = parser_eval(&tokens[..off - 1])?;
    let base = parser_eval(&tokens[off + 1..])?;
    let result = base * (1.0 - pct / 100.0);
    result.is_finite().then(|| {
        percent_card(query, format::display(result), format::copy_text(result))
    })
}

fn parse_as_percent_of(tokens: &[Token], query: &str) -> Option<CalcResult> {
    let as_idx = tokens.iter().position(|t| matches!(t, Token::Ident(n) if n == "as"))?;
    if as_idx + 2 >= tokens.len()
        || tokens[as_idx + 1] != Token::Op('%')
        || !matches!(&tokens[as_idx + 2], Token::Ident(n) if n == "of")
    {
        return None;
    }
    let x = parser_eval(&tokens[..as_idx])?;
    let y = parser_eval(&tokens[as_idx + 3..])?;
    if y == 0.0 {
        return None;
    }
    let ratio = x / y * 100.0;
    ratio.is_finite().then(|| {
        percent_card(
            query,
            format!("{}%", format::display(ratio)),
            format!("{}%", format::copy_text(ratio)),
        )
    })
}

fn percent_card(query: &str, display: String, copy: String) -> CalcResult {
    CalcResult::value(
        query.split_whitespace().collect::<Vec<_>>().join(" "),
        display,
        copy,
        Some("Expression".into()),
        Some("Result".into()),
    )
}

fn tokenize(input: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let is_digit = |ch: char| ch.is_ascii_digit();

    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        if ch == '0' && i + 2 < chars.len() {
            let radix = match chars[i + 1].to_ascii_lowercase() {
                'x' => Some(16),
                'b' => Some(2),
                'o' => Some(8),
                _ => None,
            };
            if let Some(radix) = radix {
                let start = i + 2;
                let mut end = start;
                while end < chars.len() && chars[end].is_ascii_hexdigit() {
                    end += 1;
                }
                if end > start {
                    if let Ok(value) = u64::from_str_radix(&chars[start..end].iter().collect::<String>(), radix)
                    {
                        tokens.push(Token::IntLiteral { value, radix });
                        i = end;
                        continue;
                    }
                }
            }
        }

        if is_digit(ch) || (ch == '.' && i + 1 < chars.len() && is_digit(chars[i + 1])) {
            let mut text = String::new();
            let mut seen_dot = false;
            while i < chars.len() {
                let c = chars[i];
                if is_digit(c) {
                    text.push(c);
                } else if c == ',' && i + 1 < chars.len() && is_digit(chars[i + 1]) {
                    // grouping separator between digits
                } else if c == '.' && !seen_dot {
                    seen_dot = true;
                    text.push(c);
                } else {
                    break;
                }
                i += 1;
            }
            let mut is_shorthand = false;
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut digits = i + 1;
                if digits < chars.len() && (chars[digits] == '+' || chars[digits] == '-') {
                    digits += 1;
                }
                let mut end = digits;
                while end < chars.len() && is_digit(chars[end]) {
                    end += 1;
                }
                if end > digits {
                    text.push_str(&chars[i..end].iter().collect::<String>());
                    i = end;
                    is_shorthand = true;
                }
            }
            let value: f64 = text.parse().ok()?;
            if !value.is_finite() {
                return None;
            }
            if i < chars.len() && (chars[i] == 'k' || chars[i] == 'K') && is_compact_suffix(&chars, i)
            {
                tokens.push(Token::CompactNumber(value * 1_000.0));
                i += 1;
            } else if is_shorthand {
                tokens.push(Token::CompactNumber(value));
            } else {
                tokens.push(Token::Number(value));
            }
            continue;
        }

        if ch.is_alphabetic() || ch == '°' {
            if ch.is_alphabetic() {
                let mut letter_end = i;
                while letter_end < chars.len() && chars[letter_end].is_alphabetic() {
                    letter_end += 1;
                }
                if letter_end < chars.len() && is_digit(chars[letter_end]) {
                    let prefix: String = chars[i..letter_end].iter().collect::<String>().to_lowercase();
                    if units::lookup(&prefix).is_none() && currency::lookup(&prefix).is_some() {
                        tokens.push(Token::Ident(prefix));
                        i = letter_end;
                        continue;
                    }
                }
            }
            let mut text = String::new();
            while i < chars.len() {
                let c = chars[i];
                if c.is_alphabetic() || c == '°' || is_digit(c) {
                    text.push(c);
                } else if c == '²' {
                    text.push('2');
                } else if c == '³' {
                    text.push('3');
                } else {
                    break;
                }
                i += 1;
            }
            tokens.push(Token::Ident(text.to_lowercase()));
            continue;
        }

        if let Some(code) = currency::sign_code(ch) {
            tokens.push(Token::Ident(code.into()));
            i += 1;
            continue;
        }

        if ch == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            tokens.push(Token::Op('^'));
            i += 2;
            continue;
        }

        match ch {
            '+' | '(' | ')' | '!' | '%' | '^' => tokens.push(Token::Op(ch)),
            '*' | '×' => tokens.push(Token::Op('*')),
            '/' | '÷' => tokens.push(Token::Op('/')),
            '−' => tokens.push(Token::Op('-')),
            '-' => {
                if i + 1 < chars.len() && chars[i + 1] == '>' {
                    tokens.push(Token::Arrow);
                    i += 1;
                } else {
                    tokens.push(Token::Op('-'));
                }
            }
            '→' => tokens.push(Token::Arrow),
            '=' => {
                if i != chars.len() - 1 {
                    return None;
                }
            }
            _ => return None,
        }
        i += 1;
    }
    Some(tokens)
}

fn is_compact_suffix(chars: &[char], index: usize) -> bool {
    let next = index + 1;
    if next >= chars.len() {
        return true;
    }
    if is_temperature_conversion(chars, next) {
        return false;
    }
    if !chars[next].is_alphabetic() {
        return true;
    }
    let mut end = next;
    while end < chars.len() && chars[end].is_alphabetic() {
        end += 1;
    }
    let word: String = chars[next..end].iter().collect::<String>().to_lowercase();
    currency::lookup(&word).is_some()
}

fn is_temperature_conversion(chars: &[char], from: usize) -> bool {
    let remainder: String = chars[from..].iter().collect();
    let remainder = remainder.trim().to_lowercase();
    for connector in ["to", "in", "->", "→"] {
        if let Some(rest) = remainder.strip_prefix(connector) {
            let target = rest.trim();
            if units::lookup(target).map(|u| u.category) == Some(UnitCategory::Temperature) {
                return true;
            }
        }
    }
    false
}

fn parser_eval(tokens: &[Token]) -> Option<f64> {
    let mut parser = Parser {
        tokens,
        pos: 0,
    };
    let result = parser.parse_expression(0)?;
    if !parser.is_at_end() || !result.effective().is_finite() {
        return None;
    }
    Some(result.effective())
}

fn factorial(value: f64) -> Option<f64> {
    if value < 0.0 || value.round() != value || value > 170.0 {
        return None;
    }
    let mut result = 1.0;
    let mut next = 2.0;
    while next <= value {
        result *= next;
        next += 1.0;
    }
    Some(result)
}

fn constant(name: &str) -> Option<f64> {
    match name {
        "pi" | "π" => Some(std::f64::consts::PI),
        "e" => Some(std::f64::consts::E),
        _ => None,
    }
}

fn function(name: &str) -> Option<fn(f64) -> f64> {
    Some(match name {
        "sqrt" => f64::sqrt,
        "log" => f64::log10,
        "ln" => f64::ln,
        "sin" => f64::sin,
        "cos" => f64::cos,
        "tan" => f64::tan,
        "abs" => f64::abs,
        "floor" => f64::floor,
        "ceil" => f64::ceil,
        "round" => f64::round,
        _ => return None,
    })
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

#[derive(Clone, Copy)]
struct Scalar {
    value: f64,
    is_percent: bool,
}

impl Scalar {
    fn effective(self) -> f64 {
        if self.is_percent {
            self.value / 100.0
        } else {
            self.value
        }
    }
}

struct BinaryOp {
    op: char,
    binding_power: i32,
    right_binding_power: i32,
}

impl Parser<'_> {
    const UNARY_BP: i32 = 25;
    const MUL_BP: i32 = 20;

    fn is_at_end(&self) -> bool {
        self.pos == self.tokens.len()
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn parse_expression(&mut self, min_bp: i32) -> Option<Scalar> {
        let mut lhs = self.parse_operand()?;
        loop {
            if let Some(binary) = self.peek_binary() {
                if binary.binding_power >= min_bp {
                    self.pos += 1;
                    let rhs = self.parse_expression(binary.right_binding_power)?;
                    lhs = apply_scalar(binary.op, lhs, rhs)?;
                    continue;
                }
            }
            if self.implies_multiplication() && Self::MUL_BP >= min_bp {
                let rhs = self.parse_expression(Self::MUL_BP + 1)?;
                lhs = apply_scalar('*', lhs, rhs)?;
                continue;
            }
            break;
        }
        Some(lhs)
    }

    fn implies_multiplication(&self) -> bool {
        match self.current() {
            Some(Token::Op('(')) => true,
            Some(Token::Ident(name)) => constant(name).is_some() || function(name).is_some(),
            _ => false,
        }
    }

    fn peek_binary(&self) -> Option<BinaryOp> {
        match self.current() {
            Some(Token::Op('+' | '-')) => Some(BinaryOp {
                op: match self.current() {
                    Some(Token::Op(op)) => *op,
                    _ => unreachable!(),
                },
                binding_power: 10,
                right_binding_power: 11,
            }),
            Some(Token::Op('*' | '/')) => Some(BinaryOp {
                op: match self.current() {
                    Some(Token::Op(op)) => *op,
                    _ => unreachable!(),
                },
                binding_power: Self::MUL_BP,
                right_binding_power: Self::MUL_BP + 1,
            }),
            Some(Token::Ident(name)) if name == "of" => Some(BinaryOp {
                op: '*',
                binding_power: Self::MUL_BP,
                right_binding_power: Self::MUL_BP + 1,
            }),
            Some(Token::Ident(name)) if name == "mod" => Some(BinaryOp {
                op: '%',
                binding_power: Self::MUL_BP,
                right_binding_power: Self::MUL_BP + 1,
            }),
            Some(Token::Op('^')) => Some(BinaryOp {
                op: '^',
                binding_power: 30,
                right_binding_power: 30,
            }),
            _ => None,
        }
    }

    fn parse_operand(&mut self) -> Option<Scalar> {
        let mut value = self.parse_prefix()?;
        loop {
            match self.current() {
                Some(Token::Op('!')) => {
                    if value.is_percent {
                        return None;
                    }
                    value = Scalar {
                        value: factorial(value.value)?,
                        is_percent: false,
                    };
                }
                Some(Token::Op('%')) => {
                    if value.is_percent {
                        return None;
                    }
                    value.is_percent = true;
                }
                Some(Token::Ident(name)) if name == "deg" => {
                    if value.is_percent {
                        return None;
                    }
                    value = Scalar {
                        value: value.value * std::f64::consts::PI / 180.0,
                        is_percent: false,
                    };
                }
                _ => break,
            }
            self.pos += 1;
        }
        Some(value)
    }

    fn parse_prefix(&mut self) -> Option<Scalar> {
        match self.current()? {
            Token::Number(n) | Token::CompactNumber(n) => {
                let n = *n;
                self.pos += 1;
                Some(Scalar {
                    value: n,
                    is_percent: false,
                })
            }
            Token::IntLiteral { value, .. } => {
                let n = *value as f64;
                self.pos += 1;
                Some(Scalar {
                    value: n,
                    is_percent: false,
                })
            }
            Token::Op('-') => {
                self.pos += 1;
                let operand = self.parse_expression(Self::UNARY_BP)?;
                Some(Scalar {
                    value: -operand.effective(),
                    is_percent: false,
                })
            }
            Token::Op('+') => {
                self.pos += 1;
                self.parse_expression(Self::UNARY_BP)
            }
            Token::Op('(') => {
                self.pos += 1;
                let inner = self.parse_expression(0)?;
                match self.current() {
                    Some(Token::Op(')')) => {
                        self.pos += 1;
                        Some(inner)
                    }
                    _ => None,
                }
            }
            Token::Ident(name) => {
                let name = name.clone();
                if let Some(c) = constant(&name) {
                    self.pos += 1;
                    return Some(Scalar {
                        value: c,
                        is_percent: false,
                    });
                }
                let fn_ptr = function(&name)?;
                self.pos += 1;
                let argument = if matches!(self.current(), Some(Token::Op('('))) {
                    self.pos += 1;
                    let argument = self.parse_expression(0)?;
                    match self.current() {
                        Some(Token::Op(')')) => {
                            self.pos += 1;
                            argument
                        }
                        _ => return None,
                    }
                } else {
                    self.parse_operand()?
                };
                Some(Scalar {
                    value: fn_ptr(argument.effective()),
                    is_percent: false,
                })
            }
            _ => None,
        }
    }
}

fn apply_scalar(op: char, lhs: Scalar, rhs: Scalar) -> Option<Scalar> {
    let result = match op {
        '+' => {
            if rhs.is_percent {
                lhs.effective() * (1.0 + rhs.value / 100.0)
            } else {
                lhs.effective() + rhs.effective()
            }
        }
        '-' => {
            if rhs.is_percent {
                lhs.effective() * (1.0 - rhs.value / 100.0)
            } else {
                lhs.effective() - rhs.effective()
            }
        }
        '*' => lhs.effective() * rhs.effective(),
        '/' => lhs.effective() / rhs.effective(),
        '%' => lhs.effective() % rhs.effective(),
        '^' => lhs.effective().powf(rhs.effective()),
        _ => return None,
    };
    Some(Scalar {
        value: result,
        is_percent: false,
    })
}

fn eval_quantity(
    tokens: &[Token],
    query: &str,
    rates: Option<&CurrencyRates>,
    region: Option<&str>,
    preserve_standalone_unit: bool,
) -> Option<CalcResult> {
    let split = split_conversion(tokens);
    if split.target_name.is_some() && is_simple_conversion_source(&split.expression_tokens) {
        return None;
    }

    let mut parser = QuantityParser {
        tokens: &split.expression_tokens,
        rates,
        position: 0,
        operation_count: 0,
        dimension_count: 0,
        used_currency: false,
        currency_codes: Vec::new(),
        issue: None,
    };
    let value = match parser.parse() {
        Some(value) => value,
        None => {
            return parser.issue.map(|message| CalcResult::error(query, message));
        }
    };
    if parser.dimension_count == 0 {
        return None;
    }

    if parser.used_currency {
        let Some(rates) = rates else {
            return Some(CalcResult::error(
                query,
                "Exchange rates unavailable — check your connection.",
            ));
        };
        if let Some(code) = parser
            .currency_codes
            .iter()
            .find(|code| rates.rate(code).is_none())
        {
            return Some(CalcResult::error(
                query,
                format!("No exchange rate for {code}."),
            ));
        }
    }

    if let Some(target_name) = split.target_name {
        return match parser.converted(value, &target_name) {
            Some(output) => converted_result(output, &expression_text(&split.expression_tokens)),
            None => parser.issue.map(|message| CalcResult::error(query, message)),
        };
    }

    match value.kind {
        QKind::Scalar => {
            if parser.operation_count == 0 {
                return None;
            }
            Some(CalcResult::value(
                expression_text(&split.expression_tokens),
                format::display(value.effective()),
                format::copy_text(value.effective()),
                Some("Expression".into()),
                Some("Result".into()),
            ))
        }
        QKind::Unit(unit) => {
            if !preserve_standalone_unit
                && parser.operation_count == 0
                && parser.dimension_count == 1
            {
                if let Some(Token::Ident(final_name)) = split.expression_tokens.last() {
                    if units::lookup(final_name).is_some() {
                        return None;
                    }
                }
            }
            if parser.operation_count == 0 && !preserve_standalone_unit {
                return None;
            }
            Some(measurement_result(
                value.amount,
                unit,
                expression_text(&split.expression_tokens),
            ))
        }
        QKind::Currency(definition) => {
            if parser.operation_count != 0 {
                return Some(currency_result(
                    value.amount,
                    definition,
                    expression_text(&split.expression_tokens),
                    "Expression",
                ));
            }
            let expression = format!("{} {}", format::display(value.amount), definition.code);
            if !preserve_standalone_unit {
                if let Some(target) = region_target(region, definition) {
                    if let Some(output) =
                        rates.and_then(|r| r.convert(value.amount, definition.code, target.code))
                    {
                        return Some(currency_result(
                            output,
                            target,
                            expression,
                            definition.name,
                        ));
                    }
                }
            }
            Some(currency_result(value.amount, definition, expression, "Expression"))
        }
    }
}

fn region_target(region: Option<&str>, from: CurrencyDef) -> Option<CurrencyDef> {
    let target = currency::lookup(&region?.to_lowercase())?;
    if target.code != from.code {
        Some(target)
    } else {
        None
    }
}

fn converted_result(value: QValue, expression: &str) -> Option<CalcResult> {
    match value.kind {
        QKind::Scalar => None,
        QKind::Unit(unit) => Some(measurement_result(value.amount, unit, expression.to_string())),
        QKind::Currency(definition) => Some(currency_result(
            value.amount,
            definition,
            expression.to_string(),
            "Expression",
        )),
    }
}

fn measurement_result(amount: f64, unit: UnitDef, expression: String) -> CalcResult {
    CalcResult::value(
        expression,
        format!("{} {}", format::display(amount), unit.symbol),
        format!("{} {}", format::copy_text(amount), unit.symbol),
        Some("Expression".into()),
        Some(unit.name.into()),
    )
}

fn currency_result(
    amount: f64,
    definition: CurrencyDef,
    expression: String,
    source_badge: &str,
) -> CalcResult {
    let formatted = format::currency(amount);
    CalcResult::value(
        expression,
        format!("{} {}", format::grouped(&formatted), definition.code),
        format!("{formatted} {}", definition.code),
        Some(source_badge.into()),
        Some(definition.name.into()),
    )
}

struct SplitConv {
    expression_tokens: Vec<Token>,
    target_name: Option<String>,
}

fn split_conversion(tokens: &[Token]) -> SplitConv {
    let n = tokens.len();
    if n >= 3 && is_connector(&tokens[n - 2]) {
        if let Token::Ident(target_name) = &tokens[n - 1] {
            let mut depth = 0i32;
            for token in &tokens[..n - 2] {
                match token {
                    Token::Op('(') => depth += 1,
                    Token::Op(')') => depth -= 1,
                    _ => {}
                }
            }
            if depth == 0 {
                return SplitConv {
                    expression_tokens: tokens[..n - 2].to_vec(),
                    target_name: Some(target_name.clone()),
                };
            }
        }
    }
    SplitConv {
        expression_tokens: tokens.to_vec(),
        target_name: None,
    }
}

fn is_simple_conversion_source(tokens: &[Token]) -> bool {
    match tokens.len() {
        1 => matches!(tokens[0], Token::Ident(_)),
        2 => matches!(
            (&tokens[0], &tokens[1]),
            (Token::Number(_) | Token::CompactNumber(_), Token::Ident(_))
                | (Token::Ident(_), Token::Number(_) | Token::CompactNumber(_))
        ),
        _ => false,
    }
}

fn expression_text(tokens: &[Token]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut attach_next = true;
    let add = |parts: &mut Vec<String>, piece: &str, attached: bool, attach_next: &mut bool| {
        if (*attach_next || attached) && !parts.is_empty() {
            parts.last_mut().unwrap().push_str(piece);
        } else {
            parts.push(piece.to_string());
        }
        *attach_next = false;
    };

    let mut index = 0;
    while index < tokens.len() {
        if let Token::Ident(name) = &tokens[index] {
            if units::lookup(name).is_none() {
                if let Some(definition) = currency::lookup(name) {
                    if index + 1 < tokens.len() {
                        if let Some(amount) = number_value(&tokens[index + 1]) {
                            add(
                                &mut parts,
                                &format::copy_text(amount),
                                false,
                                &mut attach_next,
                            );
                            add(&mut parts, definition.code, false, &mut attach_next);
                            index += 2;
                            continue;
                        }
                    }
                }
            }
        }

        match &tokens[index] {
            Token::Number(value) | Token::CompactNumber(value) => {
                add(&mut parts, &format::copy_text(*value), false, &mut attach_next);
            }
            Token::IntLiteral { value, .. } => {
                add(&mut parts, &value.to_string(), false, &mut attach_next);
            }
            Token::Ident(name) => {
                let piece = units::lookup(name)
                    .map(|u| u.symbol.to_string())
                    .or_else(|| currency::lookup(name).map(|c| c.code.to_string()))
                    .unwrap_or_else(|| name.clone());
                add(&mut parts, &piece, false, &mut attach_next);
            }
            Token::Op('(') => {
                add(&mut parts, "(", false, &mut attach_next);
                attach_next = true;
            }
            Token::Op(')') => add(&mut parts, ")", true, &mut attach_next),
            Token::Op('%') => add(&mut parts, "%", true, &mut attach_next),
            Token::Op('!') => add(&mut parts, "!", true, &mut attach_next),
            Token::Op('*') => add(&mut parts, "×", false, &mut attach_next),
            Token::Op('/') => add(&mut parts, "÷", false, &mut attach_next),
            Token::Op(op) => {
                add(&mut parts, &op.to_string(), false, &mut attach_next);
                if *op == '-' || *op == '+' {
                    attach_next = is_sign(index, tokens);
                }
            }
            Token::Arrow => add(&mut parts, "→", false, &mut attach_next),
        }
        index += 1;
    }
    parts.join(" ")
}

fn is_sign(index: usize, tokens: &[Token]) -> bool {
    if index == 0 {
        return true;
    }
    match &tokens[index - 1] {
        Token::Op(previous) => *previous != ')' && *previous != '%' && *previous != '!',
        _ => false,
    }
}

fn number_value(token: &Token) -> Option<f64> {
    match token {
        Token::Number(v) | Token::CompactNumber(v) => Some(*v),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum QKind {
    Scalar,
    Unit(UnitDef),
    Currency(CurrencyDef),
}

#[derive(Clone, Copy)]
struct QValue {
    amount: f64,
    kind: QKind,
    is_percent: bool,
}

impl QValue {
    fn effective(self) -> f64 {
        if self.is_percent {
            self.amount / 100.0
        } else {
            self.amount
        }
    }
}

struct QBinary {
    op: char,
    binding_power: i32,
    right_binding_power: i32,
    consumes_token: bool,
}

struct QuantityParser<'a> {
    tokens: &'a [Token],
    rates: Option<&'a CurrencyRates>,
    position: usize,
    operation_count: usize,
    dimension_count: usize,
    used_currency: bool,
    currency_codes: Vec<String>,
    issue: Option<String>,
}

impl QuantityParser<'_> {
    const UNARY_BP: i32 = 25;

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn parse(&mut self) -> Option<QValue> {
        let value = self.parse_expression(0)?;
        if self.position != self.tokens.len() || !value.effective().is_finite() {
            return None;
        }
        Some(value)
    }

    fn parse_expression(&mut self, min_bp: i32) -> Option<QValue> {
        let mut left = self.parse_operand()?;
        while let Some(binary) = self.peek_binary(left) {
            if binary.binding_power < min_bp {
                break;
            }
            if binary.consumes_token {
                self.position += 1;
            }
            self.operation_count += 1;
            let right = self.parse_expression(binary.right_binding_power)?;
            left = self.apply(binary.op, left, right, !binary.consumes_token)?;
        }
        Some(left)
    }

    fn peek_binary(&self, left: QValue) -> Option<QBinary> {
        match self.current() {
            Some(Token::Op('+' | '-')) => Some(QBinary {
                op: match self.current() {
                    Some(Token::Op(op)) => *op,
                    _ => unreachable!(),
                },
                binding_power: 10,
                right_binding_power: 11,
                consumes_token: true,
            }),
            Some(Token::Op('*' | '/')) => Some(QBinary {
                op: match self.current() {
                    Some(Token::Op(op)) => *op,
                    _ => unreachable!(),
                },
                binding_power: 20,
                right_binding_power: 21,
                consumes_token: true,
            }),
            Some(Token::Ident(name)) if name == "of" => Some(QBinary {
                op: '*',
                binding_power: 20,
                right_binding_power: 21,
                consumes_token: true,
            }),
            Some(Token::Op('^')) => Some(QBinary {
                op: '^',
                binding_power: 30,
                right_binding_power: 30,
                consumes_token: true,
            }),
            Some(Token::Op('(')) => Some(QBinary {
                op: '*',
                binding_power: 20,
                right_binding_power: 21,
                consumes_token: false,
            }),
            _ => {
                if !matches!(left.kind, QKind::Scalar) && self.starts_quantity(self.current()) {
                    Some(QBinary {
                        op: '+',
                        binding_power: 10,
                        right_binding_power: 11,
                        consumes_token: false,
                    })
                } else {
                    None
                }
            }
        }
    }

    fn apply(&mut self, op: char, left: QValue, right: QValue, implicit: bool) -> Option<QValue> {
        match op {
            '+' | '-' => self.add_or_subtract(op, left, right, implicit),
            '*' => self.multiply(left, right),
            '/' => self.divide(left, right),
            '^' => {
                if !matches!((left.kind, right.kind), (QKind::Scalar, QKind::Scalar)) {
                    return None;
                }
                let output = left.effective().powf(right.effective());
                output.is_finite().then_some(QValue {
                    amount: output,
                    kind: QKind::Scalar,
                    is_percent: false,
                })
            }
            _ => None,
        }
    }

    fn add_or_subtract(
        &mut self,
        op: char,
        left: QValue,
        right: QValue,
        implicit: bool,
    ) -> Option<QValue> {
        let direction = if op == '+' { 1.0 } else { -1.0 };
        if right.is_percent {
            let output = left.effective() * (1.0 + direction * right.amount / 100.0);
            return Some(QValue {
                amount: output,
                kind: left.kind,
                is_percent: false,
            });
        }

        match (left.kind, right.kind) {
            (QKind::Scalar, QKind::Scalar) => Some(QValue {
                amount: left.effective() + direction * right.effective(),
                kind: QKind::Scalar,
                is_percent: false,
            }),
            (QKind::Unit(lhs), QKind::Unit(rhs)) => {
                if lhs.category != rhs.category {
                    let verb = if op == '+' { "add" } else { "subtract" };
                    return self.fail(format!(
                        "Cannot {verb} {} and {}.",
                        lhs.category.display_name(),
                        rhs.category.display_name()
                    ));
                }
                if lhs.category == UnitCategory::Temperature && lhs.symbol != rhs.symbol {
                    return self.fail("Cannot combine temperatures with different units.");
                }
                if implicit {
                    let converted = units::convert(right.amount, rhs, lhs);
                    Some(QValue {
                        amount: left.amount + direction * converted,
                        kind: QKind::Unit(lhs),
                        is_percent: false,
                    })
                } else {
                    let converted = units::convert(left.amount, lhs, rhs);
                    Some(QValue {
                        amount: converted + direction * right.amount,
                        kind: QKind::Unit(rhs),
                        is_percent: false,
                    })
                }
            }
            (QKind::Currency(lhs), QKind::Currency(rhs)) => {
                if implicit {
                    let converted = self.converted_currency(right.amount, rhs, lhs)?;
                    Some(QValue {
                        amount: left.amount + direction * converted,
                        kind: QKind::Currency(lhs),
                        is_percent: false,
                    })
                } else {
                    let converted = self.converted_currency(left.amount, lhs, rhs)?;
                    Some(QValue {
                        amount: converted + direction * right.amount,
                        kind: QKind::Currency(rhs),
                        is_percent: false,
                    })
                }
            }
            (QKind::Unit(lhs), QKind::Currency(_)) => {
                let verb = if op == '+' { "add" } else { "subtract" };
                self.fail(format!(
                    "Cannot {verb} {} and {}.",
                    lhs.category.display_name(),
                    currency::CATEGORY_NAME
                ))
            }
            (QKind::Currency(_), QKind::Unit(rhs)) => {
                let verb = if op == '+' { "add" } else { "subtract" };
                self.fail(format!(
                    "Cannot {verb} {} and {}.",
                    currency::CATEGORY_NAME,
                    rhs.category.display_name()
                ))
            }
            (QKind::Unit(_), QKind::Scalar) | (QKind::Currency(_), QKind::Scalar) => {
                if implicit {
                    return None;
                }
                Some(QValue {
                    amount: left.amount + direction * right.effective(),
                    kind: left.kind,
                    is_percent: false,
                })
            }
            (QKind::Scalar, QKind::Unit(_)) | (QKind::Scalar, QKind::Currency(_)) => {
                if implicit {
                    return None;
                }
                Some(QValue {
                    amount: left.effective() + direction * right.amount,
                    kind: right.kind,
                    is_percent: false,
                })
            }
        }
    }

    fn multiply(&mut self, left: QValue, right: QValue) -> Option<QValue> {
        match (left.kind, right.kind) {
            (QKind::Scalar, QKind::Scalar) => Some(QValue {
                amount: left.effective() * right.effective(),
                kind: QKind::Scalar,
                is_percent: false,
            }),
            (QKind::Scalar, _) => Some(QValue {
                amount: left.effective() * right.effective(),
                kind: right.kind,
                is_percent: false,
            }),
            (_, QKind::Scalar) => Some(QValue {
                amount: left.effective() * right.effective(),
                kind: left.kind,
                is_percent: false,
            }),
            _ => self.fail("Multiplication of two unit values is not supported."),
        }
    }

    fn divide(&mut self, left: QValue, right: QValue) -> Option<QValue> {
        match (left.kind, right.kind) {
            (QKind::Scalar, QKind::Scalar) => {
                self.finite_division(left.effective(), right.effective(), QKind::Scalar)
            }
            (QKind::Unit(_), QKind::Scalar) | (QKind::Currency(_), QKind::Scalar) => {
                self.finite_division(left.effective(), right.effective(), left.kind)
            }
            (QKind::Scalar, QKind::Unit(_) | QKind::Currency(_)) => {
                self.fail("Division by a unit value is not supported.")
            }
            (QKind::Unit(lhs), QKind::Unit(rhs)) => {
                if lhs.category != rhs.category {
                    return self.fail(format!(
                        "Cannot divide {} by {}.",
                        lhs.category.display_name(),
                        rhs.category.display_name()
                    ));
                }
                if lhs.category == UnitCategory::Temperature {
                    return self.fail("Division of temperature values is not supported.");
                }
                let numerator = left.amount * lhs.factor;
                let denominator = right.amount * rhs.factor;
                self.finite_division(numerator, denominator, QKind::Scalar)
            }
            (QKind::Currency(lhs), QKind::Currency(rhs)) => {
                let denominator = self.converted_currency(right.amount, rhs, lhs)?;
                self.finite_division(left.amount, denominator, QKind::Scalar)
            }
            (QKind::Unit(lhs), QKind::Currency(_)) => self.fail(format!(
                "Cannot divide {} by {}.",
                lhs.category.display_name(),
                currency::CATEGORY_NAME
            )),
            (QKind::Currency(_), QKind::Unit(rhs)) => self.fail(format!(
                "Cannot divide {} by {}.",
                currency::CATEGORY_NAME,
                rhs.category.display_name()
            )),
        }
    }

    fn finite_division(&self, numerator: f64, denominator: f64, kind: QKind) -> Option<QValue> {
        let output = numerator / denominator;
        output.is_finite().then_some(QValue {
            amount: output,
            kind,
            is_percent: false,
        })
    }

    fn parse_operand(&mut self) -> Option<QValue> {
        let mut value = self.parse_prefix()?;
        loop {
            match self.current() {
                Some(Token::Ident(name)) => {
                    let name = name.clone();
                    if !matches!(value.kind, QKind::Scalar) || value.is_percent {
                        return Some(value);
                    }
                    let Some(kind) = self.dimension_named(&name) else {
                        return Some(value);
                    };
                    value.kind = kind;
                    self.dimension_count += 1;
                    self.position += 1;
                }
                Some(Token::Op('%')) => {
                    if !matches!(value.kind, QKind::Scalar) || value.is_percent {
                        return None;
                    }
                    value.is_percent = true;
                    self.position += 1;
                }
                Some(Token::Op('!')) => {
                    if !matches!(value.kind, QKind::Scalar) || value.is_percent {
                        return None;
                    }
                    value.amount = factorial(value.amount)?;
                    self.position += 1;
                }
                _ => return Some(value),
            }
        }
    }

    fn parse_prefix(&mut self) -> Option<QValue> {
        match self.current()? {
            Token::Number(v) | Token::CompactNumber(v) => {
                let v = *v;
                self.position += 1;
                Some(QValue {
                    amount: v,
                    kind: QKind::Scalar,
                    is_percent: false,
                })
            }
            Token::IntLiteral { value, .. } => {
                let v = *value as f64;
                self.position += 1;
                Some(QValue {
                    amount: v,
                    kind: QKind::Scalar,
                    is_percent: false,
                })
            }
            Token::Op('-') => {
                self.position += 1;
                let value = self.parse_expression(Self::UNARY_BP)?;
                Some(QValue {
                    amount: -value.effective(),
                    kind: value.kind,
                    is_percent: false,
                })
            }
            Token::Op('+') => {
                self.position += 1;
                self.parse_expression(Self::UNARY_BP)
            }
            Token::Op('(') => self.parse_grouped(),
            Token::Ident(name) => {
                let name = name.clone();
                if units::lookup(&name).is_some() {
                    return None;
                }
                let definition = currency::lookup(&name)?;
                let amount = self.number_at(self.position + 1)?;
                self.position += 2;
                self.record_currency(definition.code);
                self.dimension_count += 1;
                Some(QValue {
                    amount,
                    kind: QKind::Currency(definition),
                    is_percent: false,
                })
            }
            _ => None,
        }
    }

    fn parse_grouped(&mut self) -> Option<QValue> {
        let close = self.matching_parenthesis()?;
        self.position += 1;
        let target_name = self.grouped_target(close);
        let end = if target_name.is_some() { close - 2 } else { close };
        let value = self.parse_grouped_value(end)?;
        self.position = close + 1;
        match target_name {
            None => Some(value),
            Some(target_name) => {
                self.operation_count += 1;
                self.converted(value, &target_name)
            }
        }
    }

    fn parse_grouped_value(&mut self, end: usize) -> Option<QValue> {
        if end.saturating_sub(self.position) == 1 {
            if let Token::Ident(name) = &self.tokens[self.position] {
                let name = name.clone();
                if let Some(kind) = self.dimension_named(&name) {
                    self.position = end;
                    self.dimension_count += 1;
                    return Some(QValue {
                        amount: 1.0,
                        kind,
                        is_percent: false,
                    });
                }
            }
        }
        let value = self.parse_expression(0)?;
        if self.position == end {
            Some(value)
        } else {
            None
        }
    }

    fn matching_parenthesis(&self) -> Option<usize> {
        if !matches!(self.current(), Some(Token::Op('('))) {
            return None;
        }
        let mut depth = 0i32;
        for index in self.position..self.tokens.len() {
            match self.tokens[index] {
                Token::Op('(') => depth += 1,
                Token::Op(')') => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn grouped_target(&self, close: usize) -> Option<String> {
        if close.saturating_sub(self.position) < 3 {
            return None;
        }
        if !is_connector(&self.tokens[close - 2]) {
            return None;
        }
        match &self.tokens[close - 1] {
            Token::Ident(name) => Some(name.clone()),
            _ => None,
        }
    }

    fn converted(&mut self, value: QValue, target_name: &str) -> Option<QValue> {
        match value.kind {
            QKind::Scalar => None,
            QKind::Unit(from) => {
                if let Some(to) = units::lookup(target_name) {
                    if from.category != to.category {
                        return self.fail(format!(
                            "Cannot convert {} to {}.",
                            from.category.display_name(),
                            to.category.display_name()
                        ));
                    }
                    let output = units::convert(value.amount, from, to);
                    return output.is_finite().then_some(QValue {
                        amount: output,
                        kind: QKind::Unit(to),
                        is_percent: false,
                    });
                }
                if currency::lookup(target_name).is_some() {
                    return self.fail(format!(
                        "Cannot convert {} to {}.",
                        from.category.display_name(),
                        currency::CATEGORY_NAME
                    ));
                }
                None
            }
            QKind::Currency(from) => {
                if let Some(to) = currency::lookup(target_name) {
                    let output = self.converted_currency(value.amount, from, to)?;
                    return Some(QValue {
                        amount: output,
                        kind: QKind::Currency(to),
                        is_percent: false,
                    });
                }
                if let Some(to) = units::lookup(target_name) {
                    return self.fail(format!(
                        "Cannot convert {} to {}.",
                        currency::CATEGORY_NAME,
                        to.category.display_name()
                    ));
                }
                None
            }
        }
    }

    fn dimension_named(&mut self, name: &str) -> Option<QKind> {
        if let Some(unit) = units::lookup(name) {
            return Some(QKind::Unit(unit));
        }
        let definition = currency::lookup(name)?;
        self.record_currency(definition.code);
        Some(QKind::Currency(definition))
    }

    fn converted_currency(
        &mut self,
        amount: f64,
        from: CurrencyDef,
        to: CurrencyDef,
    ) -> Option<f64> {
        self.record_currency(from.code);
        self.record_currency(to.code);
        let Some(rates) = self.rates else {
            self.issue = Some("Exchange rates unavailable — check your connection.".into());
            return None;
        };
        if rates.rate(from.code).is_none() {
            self.issue = Some(format!("No exchange rate for {}.", from.code));
            return None;
        }
        if rates.rate(to.code).is_none() {
            self.issue = Some(format!("No exchange rate for {}.", to.code));
            return None;
        }
        rates.convert(amount, from.code, to.code)
    }

    fn record_currency(&mut self, code: &str) {
        self.used_currency = true;
        if !self.currency_codes.iter().any(|c| c == code) {
            self.currency_codes.push(code.to_string());
        }
    }

    fn number_at(&self, index: usize) -> Option<f64> {
        self.tokens.get(index).and_then(number_value)
    }

    fn starts_quantity(&self, token: Option<&Token>) -> bool {
        match token {
            Some(Token::Number(_) | Token::CompactNumber(_) | Token::IntLiteral { .. }) => true,
            Some(Token::Ident(name)) => {
                units::lookup(name).is_none()
                    && currency::lookup(name).is_some()
                    && self.number_at(self.position + 1).is_some()
            }
            _ => false,
        }
    }

    fn fail<T>(&mut self, message: impl Into<String>) -> Option<T> {
        self.issue = Some(message.into());
        None
    }
}
