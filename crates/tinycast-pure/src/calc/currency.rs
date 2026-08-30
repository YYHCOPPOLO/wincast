//! Currency names, signs, and an injected units-per-base rate snapshot.

use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CurrencyDef {
    pub code: &'static str,
    pub name: &'static str,
}

/// A rate snapshot in units per 1 base currency. The engine never fetches.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CurrencyRates {
    pub fetched_at: i64,
    pub units_per_base: Vec<(String, f64)>,
}

/// Last-wins merge: crypto quotes overwrite fiat on the same code.
pub fn merge_feeds(fiat: &[(&str, f64)], crypto: &[(&str, f64)]) -> CurrencyRates {
    let mut units_per_base = Vec::new();
    for (code, rate) in fiat.iter().chain(crypto.iter()) {
        upsert_rate(&mut units_per_base, code, *rate);
    }
    CurrencyRates {
        fetched_at: 0,
        units_per_base,
    }
}

/// Whole snapshots price at least one of the coins this app asks the coin feed for.
pub fn prices_coins(rates: &CurrencyRates) -> bool {
    ["BTC", "ETH", "SOL"]
        .iter()
        .any(|code| rates.rate(code).is_some())
}

/// ISO 4217 from a BCP-47 locale (`en-US`, `zh-Hans-CN`). Region is the last 2-letter subtag.
pub fn currency_for_locale(locale: &str) -> Option<&'static str> {
    region_currency(locale_region(locale)?)
}

fn locale_region(locale: &str) -> Option<[u8; 2]> {
    let tag = locale.split(['-', '_']).rev().find(|part| {
        part.len() == 2 && part.bytes().all(|b| b.is_ascii_alphabetic())
    })?;
    let bytes = tag.as_bytes();
    Some([bytes[0].to_ascii_uppercase(), bytes[1].to_ascii_uppercase()])
}

fn upsert_rate(table: &mut Vec<(String, f64)>, code: &str, rate: f64) {
    if let Some(existing) = table
        .iter_mut()
        .find(|(c, _)| c.eq_ignore_ascii_case(code))
    {
        existing.0 = code.to_string();
        existing.1 = rate;
        return;
    }
    table.push((code.to_string(), rate));
}

fn region_currency(region: [u8; 2]) -> Option<&'static str> {
    Some(match &region {
        b"AD" | b"AT" | b"AX" | b"BE" | b"CY" | b"DE" | b"EE" | b"ES" | b"EU" | b"FI" | b"FR"
        | b"GR" | b"HR" | b"IE" | b"IT" | b"LT" | b"LU" | b"LV" | b"MC" | b"ME" | b"MT" | b"NL"
        | b"PT" | b"SI" | b"SK" | b"SM" | b"VA" | b"XK" => "EUR",
        b"AE" => "AED",
        b"AF" => "AFN",
        b"AG" | b"AI" | b"DM" | b"GD" | b"KN" | b"LC" | b"MS" | b"VC" => "XCD",
        b"AL" => "ALL",
        b"AM" => "AMD",
        b"AO" => "AOA",
        b"AR" => "ARS",
        b"AS" | b"EC" | b"FM" | b"GU" | b"MH" | b"MP" | b"PR" | b"PW" | b"SV" | b"TC" | b"TL"
        | b"UM" | b"US" | b"VG" | b"VI" => "USD",
        b"AU" | b"CC" | b"CX" | b"HM" | b"KI" | b"NF" | b"NR" | b"TV" => "AUD",
        b"AW" => "AWG",
        b"AZ" => "AZN",
        b"BA" => "BAM",
        b"BB" => "BBD",
        b"BD" => "BDT",
        b"BF" | b"BJ" | b"CI" | b"GW" | b"ML" | b"NE" | b"SN" | b"TG" => "XOF",
        b"BG" => "BGN",
        b"BH" => "BHD",
        b"BI" => "BIF",
        b"BM" => "BMD",
        b"BN" => "BND",
        b"BO" => "BOB",
        b"BR" => "BRL",
        b"BS" => "BSD",
        b"BT" => "BTN",
        b"BW" => "BWP",
        b"BY" => "BYN",
        b"BZ" => "BZD",
        b"CA" => "CAD",
        b"CD" => "CDF",
        b"CF" | b"CG" | b"CM" | b"GA" | b"GQ" | b"TD" => "XAF",
        b"CH" | b"LI" => "CHF",
        b"CL" => "CLP",
        b"CN" => "CNY",
        b"CO" => "COP",
        b"CR" => "CRC",
        b"CU" => "CUP",
        b"CV" => "CVE",
        b"CZ" => "CZK",
        b"DJ" => "DJF",
        b"DK" | b"FO" | b"GL" => "DKK",
        b"DO" => "DOP",
        b"DZ" => "DZD",
        b"EG" => "EGP",
        b"ER" => "ERN",
        b"ET" => "ETB",
        b"FJ" => "FJD",
        b"FK" => "FKP",
        b"GB" => "GBP",
        b"GE" => "GEL",
        b"GG" => "GGP",
        b"GH" => "GHS",
        b"GI" => "GIP",
        b"GM" => "GMD",
        b"GN" => "GNF",
        b"GT" => "GTQ",
        b"GY" => "GYD",
        b"HK" => "HKD",
        b"HN" => "HNL",
        b"HT" => "HTG",
        b"HU" => "HUF",
        b"ID" => "IDR",
        b"IL" | b"PS" => "ILS",
        b"IM" => "IMP",
        b"IN" => "INR",
        b"IQ" => "IQD",
        b"IR" => "IRR",
        b"IS" => "ISK",
        b"JE" => "JEP",
        b"JM" => "JMD",
        b"JO" => "JOD",
        b"JP" => "JPY",
        b"KE" => "KES",
        b"KG" => "KGS",
        b"KH" => "KHR",
        b"KM" => "KMF",
        b"KP" => "KPW",
        b"KR" => "KRW",
        b"KW" => "KWD",
        b"KY" => "KYD",
        b"KZ" => "KZT",
        b"LA" => "LAK",
        b"LB" => "LBP",
        b"LK" => "LKR",
        b"LR" => "LRD",
        b"LS" => "LSL",
        b"LY" => "LYD",
        b"MA" | b"EH" => "MAD",
        b"MD" => "MDL",
        b"MG" => "MGA",
        b"MK" => "MKD",
        b"MM" => "MMK",
        b"MN" => "MNT",
        b"MO" => "MOP",
        b"MR" => "MRU",
        b"MU" => "MUR",
        b"MV" => "MVR",
        b"MW" => "MWK",
        b"MX" => "MXN",
        b"MY" => "MYR",
        b"MZ" => "MZN",
        b"NA" => "NAD",
        b"NG" => "NGN",
        b"NI" => "NIO",
        b"NO" | b"BV" | b"SJ" => "NOK",
        b"NP" => "NPR",
        b"NZ" | b"CK" | b"NU" | b"PN" | b"TK" => "NZD",
        b"OM" => "OMR",
        b"PA" => "PAB",
        b"PE" => "PEN",
        b"PG" => "PGK",
        b"PH" => "PHP",
        b"PK" => "PKR",
        b"PL" => "PLN",
        b"PY" => "PYG",
        b"QA" => "QAR",
        b"RO" => "RON",
        b"RS" => "RSD",
        b"RU" => "RUB",
        b"RW" => "RWF",
        b"SA" => "SAR",
        b"SB" => "SBD",
        b"SC" => "SCR",
        b"SD" => "SDG",
        b"SE" => "SEK",
        b"SG" => "SGD",
        b"SH" => "SHP",
        b"SL" => "SLE",
        b"SO" => "SOS",
        b"SR" => "SRD",
        b"ST" => "STN",
        b"SY" => "SYP",
        b"SZ" => "SZL",
        b"TH" => "THB",
        b"TJ" => "TJS",
        b"TM" => "TMT",
        b"TN" => "TND",
        b"TO" => "TOP",
        b"TR" => "TRY",
        b"TT" => "TTD",
        b"TW" => "TWD",
        b"TZ" => "TZS",
        b"UA" => "UAH",
        b"UG" => "UGX",
        b"UY" => "UYU",
        b"UZ" => "UZS",
        b"VE" => "VES",
        b"VN" => "VND",
        b"VU" => "VUV",
        b"WF" | b"PF" | b"NC" => "XPF",
        b"WS" => "WST",
        b"YE" => "YER",
        b"ZA" => "ZAR",
        b"ZM" => "ZMW",
        _ => return None,
    })
}

impl CurrencyRates {
    pub(crate) fn rate(&self, code: &str) -> Option<f64> {
        let rate = self
            .units_per_base
            .iter()
            .find(|(c, _)| c.eq_ignore_ascii_case(code))
            .map(|(_, r)| *r)?;
        if rate > 0.0 && rate.is_finite() {
            Some(rate)
        } else {
            None
        }
    }

    pub(crate) fn convert(&self, amount: f64, from: &str, to: &str) -> Option<f64> {
        let source = self.rate(from)?;
        let target = self.rate(to)?;
        let output = amount / source * target;
        output.is_finite().then_some(output)
    }
}

pub(crate) const CATEGORY_NAME: &str = "Currency";

pub(crate) fn lookup(name: &str) -> Option<CurrencyDef> {
    by_name().get(name).copied()
}

pub(crate) fn sign_code(ch: char) -> Option<&'static str> {
    Some(match ch {
        '؋' => "afn",
        '֏' => "amd",
        '₼' => "azn",
        '৳' => "bdt",
        '₡' => "crc",
        '€' => "eur",
        '£' => "gbp",
        '₾' => "gel",
        '₪' => "ils",
        '₹' => "inr",
        '¥' => "jpy",
        '⃀' => "kgs",
        '៛' => "khr",
        '₩' => "krw",
        '₸' => "kzt",
        '₭' => "lak",
        '₮' => "mnt",
        '₦' => "ngn",
        '₱' => "php",
        '₲' => "pyg",
        '₽' => "rub",
        '฿' => "thb",
        '₺' => "try",
        '₴' => "uah",
        '$' => "usd",
        '₫' => "vnd",
        _ => return None,
    })
}

fn by_name() -> &'static HashMap<String, CurrencyDef> {
    static TABLE: OnceLock<HashMap<String, CurrencyDef>> = OnceLock::new();
    TABLE.get_or_init(build_table)
}

fn build_table() -> HashMap<String, CurrencyDef> {
    let mut defs: HashMap<&str, CurrencyDef> = HashMap::new();
    let mut table: HashMap<String, CurrencyDef> = HashMap::new();
    for (code, name) in ALL {
        let def = CurrencyDef { code, name };
        defs.insert(code, def);
        table.insert(code.to_ascii_lowercase(), def);
    }
    for (word, code) in ALIASES {
        if let Some(def) = defs.get(code) {
            table.insert((*word).to_string(), *def);
        }
    }
    // After generated nouns, so a ticker beats one: `sol` is Solana, `soles` stays PEN.
    for (code, name, aliases) in CRYPTO {
        let def = CurrencyDef { code, name };
        defs.insert(code, def);
        table.insert(code.to_ascii_lowercase(), def);
        for word in aliases.iter() {
            table.insert((*word).to_string(), def);
        }
    }
    for (code, words) in CONTESTED {
        if let Some(def) = defs.get(code) {
            for word in words.iter() {
                table.insert((*word).to_string(), *def);
            }
        }
    }
    for (code, words) in ISO_NAMES {
        if let Some(def) = defs.get(code) {
            for word in words.iter() {
                table.insert((*word).to_string(), *def);
            }
        }
    }
    table
}

const CONTESTED: &[(&str, &[&str])] = &[
    ("USD", &["dollar", "dollars"]),
    ("CHF", &["franc", "francs"]),
    ("GBP", &["pound", "pounds"]),
    ("MXN", &["peso", "pesos"]),
    ("INR", &["rupee", "rupees"]),
    ("KES", &["shilling", "shillings"]),
    ("AED", &["dirham", "dirhams"]),
    ("KRW", &["won"]),
    ("RON", &["leu", "lei"]),
    ("RUB", &["ruble", "rubles"]),
    ("SAR", &["riyal", "riyals"]),
];

const ISO_NAMES: &[(&str, &[&str])] = &[("CNY", &["rmb", "renminbi"])];

const CRYPTO: &[(&str, &str, &[&str])] = &[
    ("ADA", "Cardano", &["cardano"]),
    ("AVAX", "Avalanche", &["avalanche"]),
    ("BCH", "Bitcoin Cash", &[]),
    ("BNB", "BNB", &["binance"]),
    ("BSV", "Bitcoin SV", &[]),
    ("BTC", "Bitcoin", &["bitcoin"]),
    ("DASH", "Dash", &[]),
    ("DOGE", "Dogecoin", &["dogecoin"]),
    ("DOT", "Polkadot", &["polkadot"]),
    ("EOS", "EOS", &[]),
    ("ETC", "Ethereum Classic", &[]),
    ("ETH", "Ethereum", &["ethereum", "ether"]),
    ("LTC", "Litecoin", &["litecoin"]),
    ("LUNA", "Terra", &["terra"]),
    ("NEO", "Neo", &[]),
    ("POL", "Polygon", &["polygon"]),
    ("SHIB", "Shiba Inu", &["shiba"]),
    ("SOL", "Solana", &["solana"]),
    ("TRX", "TRON", &["tron"]),
    ("USDT", "Tether", &["tether"]),
    ("XLM", "Stellar", &["stellar"]),
    ("XMR", "Monero", &["monero"]),
    ("XRP", "XRP", &["ripple"]),
];

const ALL: &[(&str, &str)] = &[
    ("AED", "UAE Dirham"),
    ("AFN", "Afghan Afghani"),
    ("ALL", "Albanian Lek"),
    ("AMD", "Armenian Dram"),
    ("AOA", "Angolan Kwanza"),
    ("ARS", "Argentine Peso"),
    ("AUD", "Australian Dollar"),
    ("AWG", "Aruban Florin"),
    ("AZN", "Azerbaijani Manat"),
    ("BAM", "Bosnia-Herzegovina Convertible Mark"),
    ("BBD", "Barbadian Dollar"),
    ("BDT", "Bangladeshi Taka"),
    ("BHD", "Bahraini Dinar"),
    ("BIF", "Burundian Franc"),
    ("BMD", "Bermudan Dollar"),
    ("BND", "Brunei Dollar"),
    ("BOB", "Bolivian Boliviano"),
    ("BRL", "Brazilian Real"),
    ("BSD", "Bahamian Dollar"),
    ("BTN", "Bhutanese Ngultrum"),
    ("BWP", "Botswanan Pula"),
    ("BYN", "Belarusian Ruble"),
    ("BZD", "Belize Dollar"),
    ("CAD", "Canadian Dollar"),
    ("CDF", "Congolese Franc"),
    ("CHF", "Swiss Franc"),
    ("CLF", "Chilean Unit of Account (UF)"),
    ("CLP", "Chilean Peso"),
    ("CNH", "Chinese Yuan (offshore)"),
    ("CNY", "Chinese Yuan"),
    ("COP", "Colombian Peso"),
    ("CRC", "Costa Rican Colón"),
    ("CUP", "Cuban Peso"),
    ("CVE", "Cape Verdean Escudo"),
    ("CZK", "Czech Koruna"),
    ("DJF", "Djiboutian Franc"),
    ("DKK", "Danish Krone"),
    ("DOP", "Dominican Peso"),
    ("DZD", "Algerian Dinar"),
    ("EGP", "Egyptian Pound"),
    ("ERN", "Eritrean Nakfa"),
    ("ETB", "Ethiopian Birr"),
    ("EUR", "Euro"),
    ("FJD", "Fijian Dollar"),
    ("FKP", "Falkland Islands Pound"),
    ("GBP", "British Pound"),
    ("GEL", "Georgian Lari"),
    ("GGP", "Guernsey Pound"),
    ("GHS", "Ghanaian Cedi"),
    ("GIP", "Gibraltar Pound"),
    ("GMD", "Gambian Dalasi"),
    ("GNF", "Guinean Franc"),
    ("GTQ", "Guatemalan Quetzal"),
    ("GYD", "Guyanaese Dollar"),
    ("HKD", "Hong Kong Dollar"),
    ("HNL", "Honduran Lempira"),
    ("HTG", "Haitian Gourde"),
    ("HUF", "Hungarian Forint"),
    ("IDR", "Indonesian Rupiah"),
    ("ILS", "Israeli New Shekel"),
    ("IMP", "Isle of Man Pound"),
    ("INR", "Indian Rupee"),
    ("IQD", "Iraqi Dinar"),
    ("IRR", "Iranian Rial"),
    ("ISK", "Icelandic Króna"),
    ("JEP", "Jersey Pound"),
    ("JMD", "Jamaican Dollar"),
    ("JOD", "Jordanian Dinar"),
    ("JPY", "Japanese Yen"),
    ("KES", "Kenyan Shilling"),
    ("KGS", "Kyrgyz Som"),
    ("KHR", "Cambodian Riel"),
    ("KMF", "Comorian Franc"),
    ("KPW", "North Korean Won"),
    ("KRW", "South Korean Won"),
    ("KWD", "Kuwaiti Dinar"),
    ("KYD", "Cayman Islands Dollar"),
    ("KZT", "Kazakhstani Tenge"),
    ("LAK", "Laotian Kip"),
    ("LBP", "Lebanese Pound"),
    ("LKR", "Sri Lankan Rupee"),
    ("LRD", "Liberian Dollar"),
    ("LSL", "Lesotho Loti"),
    ("LYD", "Libyan Dinar"),
    ("MAD", "Moroccan Dirham"),
    ("MDL", "Moldovan Leu"),
    ("MGA", "Malagasy Ariary"),
    ("MKD", "Macedonian Denar"),
    ("MMK", "Myanmar Kyat"),
    ("MNT", "Mongolian Tugrik"),
    ("MOP", "Macanese Pataca"),
    ("MRU", "Mauritanian Ouguiya"),
    ("MUR", "Mauritian Rupee"),
    ("MVR", "Maldivian Rufiyaa"),
    ("MWK", "Malawian Kwacha"),
    ("MXN", "Mexican Peso"),
    ("MYR", "Malaysian Ringgit"),
    ("MZN", "Mozambican Metical"),
    ("NAD", "Namibian Dollar"),
    ("NGN", "Nigerian Naira"),
    ("NIO", "Nicaraguan Córdoba"),
    ("NOK", "Norwegian Krone"),
    ("NPR", "Nepalese Rupee"),
    ("NZD", "New Zealand Dollar"),
    ("OMR", "Omani Rial"),
    ("PAB", "Panamanian Balboa"),
    ("PEN", "Peruvian Sol"),
    ("PGK", "Papua New Guinean Kina"),
    ("PHP", "Philippine Peso"),
    ("PKR", "Pakistani Rupee"),
    ("PLN", "Polish Zloty"),
    ("PYG", "Paraguayan Guarani"),
    ("QAR", "Qatari Riyal"),
    ("RON", "Romanian Leu"),
    ("RSD", "Serbian Dinar"),
    ("RUB", "Russian Ruble"),
    ("RWF", "Rwandan Franc"),
    ("SAR", "Saudi Riyal"),
    ("SBD", "Solomon Islands Dollar"),
    ("SCR", "Seychellois Rupee"),
    ("SDG", "Sudanese Pound"),
    ("SEK", "Swedish Krona"),
    ("SGD", "Singapore Dollar"),
    ("SHP", "St. Helena Pound"),
    ("SLE", "Sierra Leonean Leone"),
    ("SOS", "Somali Shilling"),
    ("SRD", "Surinamese Dollar"),
    ("STN", "São Tomé & Príncipe Dobra"),
    ("SYP", "Syrian Pound"),
    ("SZL", "Swazi Lilangeni"),
    ("THB", "Thai Baht"),
    ("TJS", "Tajikistani Somoni"),
    ("TMT", "Turkmenistani Manat"),
    ("TND", "Tunisian Dinar"),
    ("TOP", "Tongan Paʻanga"),
    ("TRY", "Turkish Lira"),
    ("TTD", "Trinidad & Tobago Dollar"),
    ("TWD", "New Taiwan Dollar"),
    ("TZS", "Tanzanian Shilling"),
    ("UAH", "Ukrainian Hryvnia"),
    ("UGX", "Ugandan Shilling"),
    ("USD", "US Dollar"),
    ("UYU", "Uruguayan Peso"),
    ("UZS", "Uzbekistani Som"),
    ("VES", "Venezuelan Bolívar"),
    ("VND", "Vietnamese Dong"),
    ("VUV", "Vanuatu Vatu"),
    ("WST", "Samoan Tala"),
    ("XAF", "Central African CFA Franc"),
    ("XAG", "Silver"),
    ("XAU", "Gold"),
    ("XCD", "East Caribbean Dollar"),
    ("XCG", "Caribbean guilder"),
    ("XDR", "Special Drawing Rights"),
    ("XOF", "West African CFA Franc"),
    ("XPF", "CFP Franc"),
    ("YER", "Yemeni Rial"),
    ("ZAR", "South African Rand"),
    ("ZMW", "Zambian Kwacha"),
];

const ALIASES: &[(&str, &str)] = &[
    ("afghani", "AFN"),
    ("afghanis", "AFN"),
    ("ariaries", "MGA"),
    ("ariary", "MGA"),
    ("baht", "THB"),
    ("balboa", "PAB"),
    ("balboas", "PAB"),
    ("birr", "ETB"),
    ("birrs", "ETB"),
    ("bolivar", "VES"),
    ("bolívar", "VES"),
    ("bolivars", "VES"),
    ("bolívars", "VES"),
    ("boliviano", "BOB"),
    ("bolivianos", "BOB"),
    ("cedi", "GHS"),
    ("cedis", "GHS"),
    ("colon", "CRC"),
    ("colón", "CRC"),
    ("colons", "CRC"),
    ("colóns", "CRC"),
    ("cordoba", "NIO"),
    ("córdoba", "NIO"),
    ("cordobas", "NIO"),
    ("córdobas", "NIO"),
    ("dalasi", "GMD"),
    ("dalasis", "GMD"),
    ("denar", "MKD"),
    ("denari", "MKD"),
    ("dobra", "STN"),
    ("dobras", "STN"),
    ("dong", "VND"),
    ("dram", "AMD"),
    ("drams", "AMD"),
    ("emalangeni", "SZL"),
    ("escudo", "CVE"),
    ("escudos", "CVE"),
    ("euro", "EUR"),
    ("euros", "EUR"),
    ("florin", "AWG"),
    ("forint", "HUF"),
    ("forints", "HUF"),
    ("gold", "XAU"),
    ("gourde", "HTG"),
    ("gourdes", "HTG"),
    ("guarani", "PYG"),
    ("guaranis", "PYG"),
    ("guilder", "XCG"),
    ("guilders", "XCG"),
    ("hryvnia", "UAH"),
    ("hryvnias", "UAH"),
    ("kina", "PGK"),
    ("kip", "LAK"),
    ("kips", "LAK"),
    ("koruna", "CZK"),
    ("korunas", "CZK"),
    ("króna", "ISK"),
    ("kronor", "SEK"),
    ("kronur", "ISK"),
    ("krónur", "ISK"),
    ("kwanza", "AOA"),
    ("kwanzas", "AOA"),
    ("kyat", "MMK"),
    ("kyats", "MMK"),
    ("lari", "GEL"),
    ("laris", "GEL"),
    ("lek", "ALL"),
    ("leke", "ALL"),
    ("lekë", "ALL"),
    ("lempira", "HNL"),
    ("lempiras", "HNL"),
    ("leone", "SLE"),
    ("leones", "SLE"),
    ("lilangeni", "SZL"),
    ("lira", "TRY"),
    ("loti", "LSL"),
    ("lotis", "LSL"),
    ("manats", "AZN"),
    ("mark", "BAM"),
    ("marks", "BAM"),
    ("metical", "MZN"),
    ("meticals", "MZN"),
    ("naira", "NGN"),
    ("nairas", "NGN"),
    ("nakfa", "ERN"),
    ("nakfas", "ERN"),
    ("ngultrum", "BTN"),
    ("ngultrums", "BTN"),
    ("ouguiya", "MRU"),
    ("ouguiyas", "MRU"),
    ("paanga", "TOP"),
    ("paʻanga", "TOP"),
    ("pataca", "MOP"),
    ("patacas", "MOP"),
    ("pula", "BWP"),
    ("pulas", "BWP"),
    ("quetzal", "GTQ"),
    ("quetzals", "GTQ"),
    ("rand", "ZAR"),
    ("real", "BRL"),
    ("reals", "BRL"),
    ("riel", "KHR"),
    ("riels", "KHR"),
    ("ringgit", "MYR"),
    ("ringgits", "MYR"),
    ("rufiyaa", "MVR"),
    ("rufiyaas", "MVR"),
    ("rupiah", "IDR"),
    ("rupiahs", "IDR"),
    ("shekel", "ILS"),
    ("shekels", "ILS"),
    ("silver", "XAG"),
    ("sol", "PEN"),
    ("soles", "PEN"),
    ("somoni", "TJS"),
    ("somonis", "TJS"),
    ("soms", "KGS"),
    ("taka", "BDT"),
    ("takas", "BDT"),
    ("tala", "WST"),
    ("tenge", "KZT"),
    ("tenges", "KZT"),
    ("tugrik", "MNT"),
    ("tugriks", "MNT"),
    ("vatu", "VUV"),
    ("vatus", "VUV"),
    ("yen", "JPY"),
    ("yuan", "CNY"),
    ("zloty", "PLN"),
    ("zlotys", "PLN"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_maps_to_iso_4217() {
        assert_eq!(currency_for_locale("en-US"), Some("USD"));
        assert_eq!(currency_for_locale("en-GB"), Some("GBP"));
        assert_eq!(currency_for_locale("bn-BD"), Some("BDT"));
        assert_eq!(currency_for_locale("zh-Hans-CN"), Some("CNY"));
        assert!(currency_for_locale("en").is_none());
    }
}
