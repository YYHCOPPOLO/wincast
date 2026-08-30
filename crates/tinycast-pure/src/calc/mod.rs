//! Pure calculator engine. Inputs that cannot be computed (clock, FX, region) are injected.

mod currency;
mod datetime;
mod engine;
mod format;
mod history;
mod units;

pub use currency::{currency_for_locale, merge_feeds, prices_coins, CurrencyRates};
pub use engine::evaluate;
pub use history::{history_copy_payload, CalcHistoryEntry, CalculatorHistoryStore};

pub fn lookup(rates: &CurrencyRates, code: &str) -> Option<f64> {
    rates.rate(code)
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalcResult {
    pub expression: String,
    pub display: String,
    pub copy_text: String,
    pub source_badge: Option<String>,
    pub target_badge: Option<String>,
}

impl CalcResult {
    pub(crate) fn value(
        expression: impl Into<String>,
        display: impl Into<String>,
        copy_text: impl Into<String>,
        source_badge: Option<String>,
        target_badge: Option<String>,
    ) -> Self {
        Self {
            expression: expression.into(),
            display: display.into(),
            copy_text: copy_text.into(),
            source_badge,
            target_badge,
        }
    }

    pub(crate) fn error(expression: impl Into<String>, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            expression: expression.into(),
            display: message.clone(),
            copy_text: message,
            source_badge: None,
            target_badge: None,
        }
    }

    pub(crate) fn with_expression(mut self, expression: String) -> Self {
        self.expression = expression;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crypto_overwrites_fiat_on_same_code() {
        let rates = merge_feeds(&[("USD", 1.0), ("BTC", 999.0)], &[("BTC", 65000.0)]);
        assert_eq!(lookup(&rates, "BTC"), Some(65000.0));
    }

    #[test]
    fn arithmetic_and_units() {
        let r = evaluate("2+2", 0, None, None).unwrap();
        assert_eq!(r.copy_text, "4");
        let r = evaluate("10km to mi", 0, None, None).unwrap();
        assert!(r.display.to_lowercase().contains("mi"));
    }

    #[test]
    fn lone_integer_is_not_a_card() {
        assert!(evaluate("100000", 0, None, None).is_none());
    }

    #[test]
    fn last_unit_typed_wins() {
        let r = evaluate("10kg + 500g", 0, None, None).unwrap();
        assert!(r.display.contains('g') || r.copy_text.contains("10500"));
    }

    #[test]
    fn compact_scientific_implicit_and_partial() {
        let r = evaluate("10k", 0, None, None).unwrap();
        assert_eq!(r.copy_text, "10000");
        assert_eq!(r.display, "10,000");
        let r = evaluate("1e5", 0, None, None).unwrap();
        assert_eq!(r.copy_text, "100000");
        assert_eq!(r.display, "100,000");
        let r = evaluate("4(2+3)", 0, None, None).unwrap();
        assert_eq!(r.copy_text, "20");
        let r = evaluate("10+", 0, None, None).unwrap();
        assert_eq!(r.copy_text, "10");
        let r = evaluate("10 +", 0, None, None).unwrap();
        assert_eq!(r.copy_text, "10");
    }

    #[test]
    fn currency_last_unit_wins_with_injected_rates() {
        let r = evaluate("$10 + €5", 0, Some(&fx()), None).unwrap();
        assert_eq!(r.copy_text, "14.20 EUR");
        assert!(r.display.contains("EUR"));
    }

    #[test]
    fn oracle_arithmetic_and_silent() {
        assert_display("2+2", "4");
        assert_display("5*7", "35");
        assert_display("2^3^2", "512");
        assert_display("2**10", "1,024");
        assert_display("(5+2)*3", "21");
        assert_display("5!", "120");
        assert_display("-2^2", "-4");
        assert_display("1/3", "0.3333333333");
        assert_display("1,000 + 234", "1,234");
        assert_display("10k + 500", "10,500");
        assert_display("1e6 + 1", "1,000,001");
        assert_display("sqrt(64)", "8");
        assert_display("sqrt 64 + 36", "44");
        assert_display("sin(30deg)", "0.5");
        assert_display("2*pi", "6.283185307");
        assert_display("(2+3)(2+3)", "25");
        assert_display("2pi", "6.283185307");
        assert_display("2sqrt(9)", "6");
        assert_display("3e+2", "300");
        assert_eq!(copy("1e-5"), "1e-05");
        assert!(evaluate("1e400", 0, None, None).is_none());
        assert_display("20% of 450", "90");
        assert_display("450 + 20%", "540");
        assert_display("-10 mod 3", "-1");
        assert_display("10 km in miles", "6.213711922 mi");
        assert_display("100 C to F", "212 °F");
        assert_display("273.15K to C", "0 °C");
        assert_display("10 k to c", "-263.15 °C");
        assert_display("255 to hex", "0xFF");
        assert_display("0xff", "255");
        assert_error("10kg to sec", "Cannot convert Weight to Time.");
        assert!(evaluate("45", 0, None, None).is_none());
        assert!(evaluate("10km to", 0, None, None).is_none());
        assert!(evaluate("10 to mi", 0, None, None).is_none());
        assert_display("45+", "45");
        assert_eq!(expression("3*3"), "3×3");
        assert_eq!(expression("10km to mi"), "10 km");
        assert_display("1m", "3 feet 3.37007874 inches");
        assert_display("1hr", "60 min");
        assert!(evaluate("5 k", 0, None, None).is_none());
        assert_display("10kg + 10g", "10,010 g");
        assert_display("5 feet 3 inches", "5.25 ft");
        assert_display("1hr 30min", "1.5 hr");
        assert_display("5feet + 1m", "2.524 m");
        assert_display("1kg + 500g + 2lb", "5.306933933 lb");
        assert_error("1kg + 1m", "Cannot add Weight and Length.");
        assert_display("5kg+5", "10 kg");
        assert!(evaluate("1hr 30", 0, None, None).is_none());
        assert_display("10kg +", "10 kg");
        assert_eq!(expression("10kg +"), "10 kg +");
        assert!(evaluate("10 of", 0, None, None).is_none());
        assert_eq!(expression("10km to mi *"), "10km to mi ×");
        assert_display("20% off 500", "400");
        assert_display("50 as % of 200", "25%");
        assert_display("day s", "86,400 s");
    }

    #[test]
    fn oracle_datetime_fixed_clock() {
        // Fri 2026-07-24 00:18:00 UTC
        const CLOCK: i64 = 1_784_852_280;
        let r = evaluate("hrs till 9am", CLOCK, None, None).unwrap();
        assert_eq!(r.display, "8.7 hours");
        assert_eq!(r.source_badge.as_deref(), Some("12:18 AM"));
        assert_eq!(r.target_badge.as_deref(), Some("9:00 AM"));
        let r = evaluate("days till 9april", CLOCK, None, None).unwrap();
        assert_eq!(r.display, "259 days");
        assert_eq!(r.source_badge.as_deref(), Some("Friday, 24 July"));
        assert_eq!(r.target_badge.as_deref(), Some("Friday, 9 April, 2027"));
        let plus_weeks = evaluate("today + 3 weeks", CLOCK, None, None).unwrap();
        assert_eq!(plus_weeks.display, "Friday, 14 August");
        assert_eq!(plus_weeks.copy_text, "Friday, 14 August");
        assert_eq!(
            history_copy_payload(&plus_weeks.copy_text),
            "Friday, 14 August"
        );
        assert_eq!(
            evaluate("now + 90 min", CLOCK, None, None).unwrap().display,
            "Friday, 24 July at 1:48 AM"
        );
        assert_eq!(
            evaluate("jul 4 - today", CLOCK, None, None)
                .unwrap()
                .display,
            "345 days"
        );
        assert_eq!(
            evaluate("5/2 - 1/2", CLOCK, None, None).unwrap().display,
            "2"
        );
        assert_eq!(
            evaluate("9/4 - today", CLOCK, None, None).unwrap().display,
            "42 days"
        );
        assert!(evaluate("today", CLOCK, None, None).is_none());
        let r = evaluate("days since 9jul", CLOCK, None, None).unwrap();
        assert_eq!(r.display, "15 days");
    }

    #[test]
    fn oracle_currency_injected_rates() {
        let rates = fx();
        let ev = |q: &str| evaluate(q, 0, Some(&rates), None).unwrap();
        assert_eq!(ev("1 euro to dollars").display, "1.09 USD");
        assert_eq!(ev("€20 to GBP").display, "17.17 GBP");
        assert_eq!(ev("20€ to GBP").display, "17.17 GBP");
        assert_eq!(ev("$100 to yen").display, "15,700.00 JPY");
        assert_eq!(ev("10 pounds to kilograms").display, "4.5359237 kg");
        assert_eq!(ev("10 pounds to euros").display, "11.65 EUR");
        assert_eq!(ev("1 cup to ml").display, "236.5882365 mL");
        assert_eq!(ev("$10 + €5").display, "14.20 EUR");
        assert_eq!(ev("€5 + $10").display, "15.43 USD");
        assert_eq!(ev("$10 + €5").expression, "10 USD + 5 EUR");
        assert_eq!(ev("(20 sgd to usd) * 30").display, "444.44 USD");
        assert_eq!(ev("1 btc to usd").display, "60,000.00 USD");
        assert_eq!(ev("1 usd to btc").copy_text, "0.00001667 BTC");
        assert_eq!(ev("1 usd").display, "1.00 USD");
        assert_eq!(
            evaluate("1 usd", 0, Some(&rates), Some("INR"))
                .unwrap()
                .display,
            "83.50 INR"
        );
        assert_eq!(
            evaluate("$10 +", 0, Some(&rates), None).unwrap().display,
            "10.00 USD"
        );
        let err = evaluate("10 usd to kg", 0, Some(&rates), None).unwrap();
        assert_eq!(err.display, "Cannot convert Currency to Weight.");
        let err = evaluate("5 usd to npr", 0, Some(&rates), None).unwrap();
        assert_eq!(err.display, "No exchange rate for NPR.");
        assert!(evaluate("usd", 0, Some(&rates), None).is_none());
        assert!(evaluate("1 krona to usd", 0, Some(&rates), None).is_none());
        let err = evaluate("$10 + $5", 0, None, None).unwrap();
        assert_eq!(
            err.display,
            "Exchange rates unavailable — check your connection."
        );
    }

    fn fx() -> CurrencyRates {
        CurrencyRates {
            fetched_at: 1_785_000_000,
            units_per_base: vec![
                ("USD".into(), 1.0),
                ("EUR".into(), 0.92),
                ("GBP".into(), 0.79),
                ("JPY".into(), 157.0),
                ("INR".into(), 83.5),
                ("CAD".into(), 1.36),
                ("KRW".into(), 1330.0),
                ("IDR".into(), 18053.0),
                ("CHF".into(), 0.81),
                ("AED".into(), 3.6725),
                ("SGD".into(), 1.35),
                ("BTC".into(), 1.0 / 60_000.0),
                ("ETH".into(), 1.0 / 2_000.0),
                ("SOL".into(), 1.0 / 100.0),
                ("DOGE".into(), 10.0),
            ],
        }
    }

    fn assert_display(query: &str, expected: &str) {
        let got = evaluate(query, 0, None, None)
            .unwrap_or_else(|| panic!("{query}: expected {expected}, got nil"));
        assert_eq!(got.display, expected, "{query}");
    }

    fn assert_error(query: &str, expected: &str) {
        let got = evaluate(query, 0, None, None)
            .unwrap_or_else(|| panic!("{query}: expected error {expected}, got nil"));
        assert_eq!(got.display, expected, "{query}");
        assert!(
            got.source_badge.is_none() && got.target_badge.is_none(),
            "{query}"
        );
    }

    fn copy(query: &str) -> String {
        evaluate(query, 0, None, None).unwrap().copy_text
    }

    fn expression(query: &str) -> String {
        evaluate(query, 0, None, None).unwrap().expression
    }
}
