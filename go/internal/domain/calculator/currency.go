package calculator

import (
	"math"
	"strings"
	"sync"
)

type CurrencyDef struct {
	Code string
	Name string
}

type RatePair struct {
	Code string
	Rate float64
}

// Rates is a snapshot in units per 1 base currency. The engine never fetches.
type Rates struct {
	FetchedAt    int64
	UnitsPerBase map[string]float64
}

func MergeFeeds(fiat, crypto []RatePair) Rates {
	rates := Rates{FetchedAt: 0, UnitsPerBase: make(map[string]float64, len(fiat)+len(crypto))}
	for _, pair := range fiat {
		rates.UnitsPerBase[strings.ToUpper(pair.Code)] = pair.Rate
	}
	for _, pair := range crypto {
		rates.UnitsPerBase[strings.ToUpper(pair.Code)] = pair.Rate
	}
	return rates
}

func PricesCoins(rates *Rates) bool {
	if rates == nil {
		return false
	}
	for _, code := range []string{"BTC", "ETH", "SOL"} {
		if _, ok := rates.Rate(code); ok {
			return true
		}
	}
	return false
}

func (r *Rates) Rate(code string) (float64, bool) {
	if r == nil {
		return 0, false
	}
	rate, ok := r.UnitsPerBase[strings.ToUpper(code)]
	if !ok || rate <= 0 || math.IsInf(rate, 0) || math.IsNaN(rate) {
		return 0, false
	}
	return rate, true
}

func (r *Rates) Convert(amount float64, from, to string) (float64, bool) {
	source, ok := r.Rate(from)
	if !ok {
		return 0, false
	}
	target, ok := r.Rate(to)
	if !ok {
		return 0, false
	}
	output := amount / source * target
	return output, !math.IsInf(output, 0) && !math.IsNaN(output)
}

func lookupCurrency(name string) (CurrencyDef, bool) {
	def, ok := currencyTable()[name]
	return def, ok
}

func signCode(r rune) string {
	return signCodes[r]
}

const currencyCategoryName = "Currency"

var (
	currencyOnce  sync.Once
	currencyCache map[string]CurrencyDef
)

var signCodes = map[rune]string{
	'؋': "afn", '֏': "amd", '₼': "azn", '৳': "bdt", '₡': "crc", '€': "eur",
	'£': "gbp", '₾': "gel", '₪': "ils", '₹': "inr", '¥': "jpy", '⃀': "kgs",
	'៛': "khr", '₩': "krw", '₸': "kzt", '₭': "lak", '₮': "mnt", '₦': "ngn",
	'₱': "php", '₲': "pyg", '₽': "rub", '฿': "thb", '₺': "try", '₴': "uah",
	'$': "usd", '₫': "vnd",
}

func currencyTable() map[string]CurrencyDef {
	currencyOnce.Do(func() { currencyCache = buildCurrencyTable() })
	return currencyCache
}

func buildCurrencyTable() map[string]CurrencyDef {
	defs := make(map[string]CurrencyDef)
	table := make(map[string]CurrencyDef)
	for _, item := range allCurrencies {
		def := CurrencyDef{Code: item[0], Name: item[1]}
		defs[item[0]] = def
		table[strings.ToLower(item[0])] = def
	}
	for name, code := range currencyAliases {
		if def, ok := defs[code]; ok {
			table[name] = def
		}
	}
	for _, item := range cryptoCurrencies {
		def := CurrencyDef{Code: item.code, Name: item.name}
		defs[item.code] = def
		table[strings.ToLower(item.code)] = def
		for _, alias := range item.aliases {
			table[alias] = def
		}
	}
	for code, words := range contestedCurrencies {
		if def, ok := defs[code]; ok {
			for _, word := range words {
				table[word] = def
			}
		}
	}
	if def, ok := defs["CNY"]; ok {
		table["rmb"] = def
		table["renminbi"] = def
	}
	return table
}

var allCurrencies = [][2]string{
	{"AED", "UAE Dirham"}, {"AFN", "Afghan Afghani"}, {"ALL", "Albanian Lek"},
	{"AMD", "Armenian Dram"}, {"AOA", "Angolan Kwanza"}, {"ARS", "Argentine Peso"},
	{"AUD", "Australian Dollar"}, {"AWG", "Aruban Florin"}, {"AZN", "Azerbaijani Manat"},
	{"BAM", "Bosnia-Herzegovina Convertible Mark"}, {"BBD", "Barbadian Dollar"},
	{"BDT", "Bangladeshi Taka"}, {"BHD", "Bahraini Dinar"}, {"BIF", "Burundian Franc"},
	{"BMD", "Bermudan Dollar"}, {"BND", "Brunei Dollar"}, {"BOB", "Bolivian Boliviano"},
	{"BRL", "Brazilian Real"}, {"BSD", "Bahamian Dollar"}, {"BTN", "Bhutanese Ngultrum"},
	{"BWP", "Botswanan Pula"}, {"BYN", "Belarusian Ruble"}, {"BZD", "Belize Dollar"},
	{"CAD", "Canadian Dollar"}, {"CDF", "Congolese Franc"}, {"CHF", "Swiss Franc"},
	{"CLF", "Chilean Unit of Account (UF)"}, {"CLP", "Chilean Peso"},
	{"CNH", "Chinese Yuan (offshore)"}, {"CNY", "Chinese Yuan"}, {"COP", "Colombian Peso"},
	{"CRC", "Costa Rican Colón"}, {"CUP", "Cuban Peso"}, {"CVE", "Cape Verdean Escudo"},
	{"CZK", "Czech Koruna"}, {"DJF", "Djiboutian Franc"}, {"DKK", "Danish Krone"},
	{"DOP", "Dominican Peso"}, {"DZD", "Algerian Dinar"}, {"EGP", "Egyptian Pound"},
	{"ERN", "Eritrean Nakfa"}, {"ETB", "Ethiopian Birr"}, {"EUR", "Euro"},
	{"FJD", "Fijian Dollar"}, {"FKP", "Falkland Islands Pound"}, {"GBP", "British Pound"},
	{"GEL", "Georgian Lari"}, {"GGP", "Guernsey Pound"}, {"GHS", "Ghanaian Cedi"},
	{"GIP", "Gibraltar Pound"}, {"GMD", "Gambian Dalasi"}, {"GNF", "Guinean Franc"},
	{"GTQ", "Guatemalan Quetzal"}, {"GYD", "Guyanaese Dollar"}, {"HKD", "Hong Kong Dollar"},
	{"HNL", "Honduran Lempira"}, {"HTG", "Haitian Gourde"}, {"HUF", "Hungarian Forint"},
	{"IDR", "Indonesian Rupiah"}, {"ILS", "Israeli New Shekel"}, {"IMP", "Isle of Man Pound"},
	{"INR", "Indian Rupee"}, {"IQD", "Iraqi Dinar"}, {"IRR", "Iranian Rial"},
	{"ISK", "Icelandic Króna"}, {"JEP", "Jersey Pound"}, {"JMD", "Jamaican Dollar"},
	{"JOD", "Jordanian Dinar"}, {"JPY", "Japanese Yen"}, {"KES", "Kenyan Shilling"},
	{"KGS", "Kyrgyz Som"}, {"KHR", "Cambodian Riel"}, {"KMF", "Comorian Franc"},
	{"KPW", "North Korean Won"}, {"KRW", "South Korean Won"}, {"KWD", "Kuwaiti Dinar"},
	{"KYD", "Cayman Islands Dollar"}, {"KZT", "Kazakhstani Tenge"}, {"LAK", "Laotian Kip"},
	{"LBP", "Lebanese Pound"}, {"LKR", "Sri Lankan Rupee"}, {"LRD", "Liberian Dollar"},
	{"LSL", "Lesotho Loti"}, {"LYD", "Libyan Dinar"}, {"MAD", "Moroccan Dirham"},
	{"MDL", "Moldovan Leu"}, {"MGA", "Malagasy Ariary"}, {"MKD", "Macedonian Denar"},
	{"MMK", "Myanmar Kyat"}, {"MNT", "Mongolian Tugrik"}, {"MOP", "Macanese Pataca"},
	{"MRU", "Mauritanian Ouguiya"}, {"MUR", "Mauritian Rupee"}, {"MVR", "Maldivian Rufiyaa"},
	{"MWK", "Malawian Kwacha"}, {"MXN", "Mexican Peso"}, {"MYR", "Malaysian Ringgit"},
	{"MZN", "Mozambican Metical"}, {"NAD", "Namibian Dollar"}, {"NGN", "Nigerian Naira"},
	{"NIO", "Nicaraguan Córdoba"}, {"NOK", "Norwegian Krone"}, {"NPR", "Nepalese Rupee"},
	{"NZD", "New Zealand Dollar"}, {"OMR", "Omani Rial"}, {"PAB", "Panamanian Balboa"},
	{"PEN", "Peruvian Sol"}, {"PGK", "Papua New Guinean Kina"}, {"PHP", "Philippine Peso"},
	{"PKR", "Pakistani Rupee"}, {"PLN", "Polish Zloty"}, {"PYG", "Paraguayan Guarani"},
	{"QAR", "Qatari Riyal"}, {"RON", "Romanian Leu"}, {"RSD", "Serbian Dinar"},
	{"RUB", "Russian Ruble"}, {"RWF", "Rwandan Franc"}, {"SAR", "Saudi Riyal"},
	{"SBD", "Solomon Islands Dollar"}, {"SCR", "Seychellois Rupee"}, {"SDG", "Sudanese Pound"},
	{"SEK", "Swedish Krona"}, {"SGD", "Singapore Dollar"}, {"SHP", "St. Helena Pound"},
	{"SLE", "Sierra Leonean Leone"}, {"SOS", "Somali Shilling"}, {"SRD", "Surinamese Dollar"},
	{"STN", "São Tomé & Príncipe Dobra"}, {"SYP", "Syrian Pound"}, {"SZL", "Swazi Lilangeni"},
	{"THB", "Thai Baht"}, {"TJS", "Tajikistani Somoni"}, {"TMT", "Turkmenistani Manat"},
	{"TND", "Tunisian Dinar"}, {"TOP", "Tongan Paʻanga"}, {"TRY", "Turkish Lira"},
	{"TTD", "Trinidad & Tobago Dollar"}, {"TWD", "New Taiwan Dollar"}, {"TZS", "Tanzanian Shilling"},
	{"UAH", "Ukrainian Hryvnia"}, {"UGX", "Ugandan Shilling"}, {"USD", "US Dollar"},
	{"UYU", "Uruguayan Peso"}, {"UZS", "Uzbekistani Som"}, {"VES", "Venezuelan Bolívar"},
	{"VND", "Vietnamese Dong"}, {"VUV", "Vanuatu Vatu"}, {"WST", "Samoan Tala"},
	{"XAF", "Central African CFA Franc"}, {"XAG", "Silver"}, {"XAU", "Gold"},
	{"XCD", "East Caribbean Dollar"}, {"XCG", "Caribbean guilder"}, {"XDR", "Special Drawing Rights"},
	{"XOF", "West African CFA Franc"}, {"XPF", "CFP Franc"}, {"YER", "Yemeni Rial"},
	{"ZAR", "South African Rand"}, {"ZMW", "Zambian Kwacha"},
}

type cryptoDef struct {
	code, name string
	aliases    []string
}

var cryptoCurrencies = []cryptoDef{
	{"ADA", "Cardano", []string{"cardano"}},
	{"AVAX", "Avalanche", []string{"avalanche"}},
	{"BCH", "Bitcoin Cash", nil},
	{"BNB", "BNB", []string{"binance"}},
	{"BSV", "Bitcoin SV", nil},
	{"BTC", "Bitcoin", []string{"bitcoin"}},
	{"DASH", "Dash", nil},
	{"DOGE", "Dogecoin", []string{"dogecoin"}},
	{"DOT", "Polkadot", []string{"polkadot"}},
	{"EOS", "EOS", nil},
	{"ETC", "Ethereum Classic", nil},
	{"ETH", "Ethereum", []string{"ethereum", "ether"}},
	{"LTC", "Litecoin", []string{"litecoin"}},
	{"LUNA", "Terra", []string{"terra"}},
	{"NEO", "Neo", nil},
	{"POL", "Polygon", []string{"polygon"}},
	{"SHIB", "Shiba Inu", []string{"shiba"}},
	{"SOL", "Solana", []string{"solana"}},
	{"TRX", "TRON", []string{"tron"}},
	{"USDT", "Tether", []string{"tether"}},
	{"XLM", "Stellar", []string{"stellar"}},
	{"XMR", "Monero", []string{"monero"}},
	{"XRP", "XRP", []string{"ripple"}},
}

var contestedCurrencies = map[string][]string{
	"USD": {"dollar", "dollars"},
	"CHF": {"franc", "francs"},
	"GBP": {"pound", "pounds"},
	"MXN": {"peso", "pesos"},
	"INR": {"rupee", "rupees"},
	"KES": {"shilling", "shillings"},
	"AED": {"dirham", "dirhams"},
	"KRW": {"won"},
	"RON": {"leu", "lei"},
	"RUB": {"ruble", "rubles"},
	"SAR": {"riyal", "riyals"},
}

var currencyAliases = map[string]string{
	"afghani": "AFN", "afghanis": "AFN", "ariaries": "MGA", "ariary": "MGA",
	"baht": "THB", "balboa": "PAB", "balboas": "PAB", "birr": "ETB", "birrs": "ETB",
	"bolivar": "VES", "bolívar": "VES", "bolivars": "VES", "bolívars": "VES",
	"boliviano": "BOB", "bolivianos": "BOB", "cedi": "GHS", "cedis": "GHS",
	"colon": "CRC", "colón": "CRC", "colons": "CRC", "colóns": "CRC",
	"cordoba": "NIO", "córdoba": "NIO", "cordobas": "NIO", "córdobas": "NIO",
	"dalasi": "GMD", "dalasis": "GMD", "denar": "MKD", "denari": "MKD",
	"dobra": "STN", "dobras": "STN", "dong": "VND", "dram": "AMD", "drams": "AMD",
	"emalangeni": "SZL", "escudo": "CVE", "escudos": "CVE", "euro": "EUR", "euros": "EUR",
	"florin": "AWG", "forint": "HUF", "forints": "HUF", "gold": "XAU",
	"gourde": "HTG", "gourdes": "HTG", "guarani": "PYG", "guaranis": "PYG",
	"guilder": "XCG", "guilders": "XCG", "hryvnia": "UAH", "hryvnias": "UAH",
	"kina": "PGK", "kip": "LAK", "kips": "LAK", "koruna": "CZK", "korunas": "CZK",
	"króna": "ISK", "kronor": "SEK", "kronur": "ISK", "krónur": "ISK",
	"kwanza": "AOA", "kwanzas": "AOA", "kyat": "MMK", "kyats": "MMK",
	"lari": "GEL", "laris": "GEL", "lek": "ALL", "leke": "ALL", "lekë": "ALL",
	"lempira": "HNL", "lempiras": "HNL", "leone": "SLE", "leones": "SLE",
	"lilangeni": "SZL", "lira": "TRY", "loti": "LSL", "lotis": "LSL",
	"manats": "AZN", "mark": "BAM", "marks": "BAM", "metical": "MZN", "meticals": "MZN",
	"naira": "NGN", "nairas": "NGN", "nakfa": "ERN", "nakfas": "ERN",
	"ngultrum": "BTN", "ngultrums": "BTN", "ouguiya": "MRU", "ouguiyas": "MRU",
	"paanga": "TOP", "paʻanga": "TOP", "pataca": "MOP", "patacas": "MOP",
	"pula": "BWP", "pulas": "BWP", "quetzal": "GTQ", "quetzals": "GTQ",
	"rand": "ZAR", "real": "BRL", "reals": "BRL", "riel": "KHR", "riels": "KHR",
	"ringgit": "MYR", "ringgits": "MYR", "rufiyaa": "MVR", "rufiyaas": "MVR",
	"rupiah": "IDR", "rupiahs": "IDR", "shekel": "ILS", "shekels": "ILS",
	"silver": "XAG", "sol": "PEN", "soles": "PEN", "somoni": "TJS", "somonis": "TJS",
	"soms": "KGS", "taka": "BDT", "takas": "BDT", "tala": "WST", "tenge": "KZT",
	"tenges": "KZT", "tugrik": "MNT", "tugriks": "MNT", "vatu": "VUV", "vatus": "VUV",
	"yen": "JPY", "yuan": "CNY", "zloty": "PLN", "zlotys": "PLN",
}

// CurrencyForLocale maps a BCP-47 locale to an ISO 4217 code.
func CurrencyForLocale(locale string) string {
	parts := strings.FieldsFunc(locale, func(r rune) bool { return r == '-' || r == '_' })
	region := ""
	for i := len(parts) - 1; i >= 0; i-- {
		if len(parts[i]) == 2 {
			region = strings.ToUpper(parts[i])
			break
		}
	}
	return regionCurrencies[region]
}

var regionCurrencies = map[string]string{
	"AD": "EUR", "AT": "EUR", "AX": "EUR", "BE": "EUR", "CY": "EUR", "DE": "EUR",
	"EE": "EUR", "ES": "EUR", "EU": "EUR", "FI": "EUR", "FR": "EUR", "GR": "EUR",
	"HR": "EUR", "IE": "EUR", "IT": "EUR", "LT": "EUR", "LU": "EUR", "LV": "EUR",
	"MC": "EUR", "ME": "EUR", "MT": "EUR", "NL": "EUR", "PT": "EUR", "SI": "EUR",
	"SK": "EUR", "SM": "EUR", "VA": "EUR", "XK": "EUR", "AE": "AED", "AF": "AFN",
	"AG": "XCD", "AI": "XCD", "DM": "XCD", "GD": "XCD", "KN": "XCD", "LC": "XCD",
	"MS": "XCD", "VC": "XCD", "AL": "ALL", "AM": "AMD", "AO": "AOA", "AR": "ARS",
	"AS": "USD", "EC": "USD", "FM": "USD", "GU": "USD", "MH": "USD", "MP": "USD",
	"PR": "USD", "PW": "USD", "SV": "USD", "TC": "USD", "TL": "USD", "UM": "USD",
	"US": "USD", "VG": "USD", "VI": "USD", "AU": "AUD", "CC": "AUD", "CX": "AUD",
	"HM": "AUD", "KI": "AUD", "NF": "AUD", "NR": "AUD", "TV": "AUD", "AW": "AWG",
	"AZ": "AZN", "BA": "BAM", "BB": "BBD", "BD": "BDT", "BF": "XOF", "BJ": "XOF",
	"CI": "XOF", "GW": "XOF", "ML": "XOF", "NE": "XOF", "SN": "XOF", "TG": "XOF",
	"BG": "BGN", "BH": "BHD", "BI": "BIF", "BM": "BMD", "BN": "BND", "BO": "BOB",
	"BR": "BRL", "BS": "BSD", "BT": "BTN", "BW": "BWP", "BY": "BYN", "BZ": "BZD",
	"CA": "CAD", "CD": "CDF", "CF": "XAF", "CG": "XAF", "CM": "XAF", "GA": "XAF",
	"GQ": "XAF", "TD": "XAF", "CH": "CHF", "LI": "CHF", "CL": "CLP", "CN": "CNY",
	"CO": "COP", "CR": "CRC", "CU": "CUP", "CV": "CVE", "CZ": "CZK", "DJ": "DJF",
	"DK": "DKK", "FO": "DKK", "GL": "DKK", "DO": "DOP", "DZ": "DZD", "EG": "EGP",
	"ER": "ERN", "ET": "ETB", "FJ": "FJD", "FK": "FKP", "GB": "GBP", "GE": "GEL",
	"GG": "GGP", "GH": "GHS", "GI": "GIP", "GM": "GMD", "GN": "GNF", "GT": "GTQ",
	"GY": "GYD", "HK": "HKD", "HN": "HNL", "HT": "HTG", "HU": "HUF", "ID": "IDR",
	"IL": "ILS", "PS": "ILS", "IM": "IMP", "IN": "INR", "IQ": "IQD", "IR": "IRR",
	"IS": "ISK", "JE": "JEP", "JM": "JMD", "JO": "JOD", "JP": "JPY", "KE": "KES",
	"KG": "KGS", "KH": "KHR", "KM": "KMF", "KP": "KPW", "KR": "KRW", "KW": "KWD",
	"KY": "KYD", "KZ": "KZT", "LA": "LAK", "LB": "LBP", "LK": "LKR", "LR": "LRD",
	"LS": "LSL", "LY": "LYD", "MA": "MAD", "EH": "MAD", "MD": "MDL", "MG": "MGA",
	"MK": "MKD", "MM": "MMK", "MN": "MNT", "MO": "MOP", "MR": "MRU", "MU": "MUR",
	"MV": "MVR", "MW": "MWK", "MX": "MXN", "MY": "MYR", "MZ": "MZN", "NA": "NAD",
	"NG": "NGN", "NI": "NIO", "NO": "NOK", "BV": "NOK", "SJ": "NOK", "NP": "NPR",
	"NZ": "NZD", "CK": "NZD", "NU": "NZD", "PN": "NZD", "TK": "NZD", "OM": "OMR",
	"PA": "PAB", "PE": "PEN", "PG": "PGK", "PH": "PHP", "PK": "PKR", "PL": "PLN",
	"PY": "PYG", "QA": "QAR", "RO": "RON", "RS": "RSD", "RU": "RUB", "RW": "RWF",
	"SA": "SAR", "SB": "SBD", "SC": "SCR", "SD": "SDG", "SE": "SEK", "SG": "SGD",
	"SH": "SHP", "SL": "SLE", "SO": "SOS", "SR": "SRD", "ST": "STN", "SY": "SYP",
	"SZ": "SZL", "TH": "THB", "TJ": "TJS", "TM": "TMT", "TN": "TND", "TO": "TOP",
	"TR": "TRY", "TT": "TTD", "TW": "TWD", "TZ": "TZS", "UA": "UAH", "UG": "UGX",
	"UY": "UYU", "UZ": "UZS", "VE": "VES", "VN": "VND", "VU": "VUV", "WF": "XPF",
	"PF": "XPF", "NC": "XPF", "WS": "WST", "YE": "YER", "ZA": "ZAR", "ZM": "ZMW",
}
