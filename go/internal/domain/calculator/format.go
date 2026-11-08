package calculator

import (
	"math"
	"strconv"
	"strings"
)

const maxExactInteger = 9_007_199_254_740_992.0

// Display is the human-facing value: ten significant digits at most, trailing
// zeros trimmed, and thousands separators for the integer part.
func Display(value float64) string {
	return Grouped(CopyText(value))
}

// CopyText is the same rounding without grouping, as pasted to the clipboard.
func CopyText(value float64) string {
	if value == 0 {
		value = 0
	}
	if math.Round(value) == value && math.Abs(value) <= maxExactInteger {
		return strconv.FormatFloat(value, 'f', 0, 64)
	}
	return formatG(value, 10)
}

// Currency formats money with two decimals, widening below one cent.
func Currency(value float64) string {
	magnitude := math.Abs(value)
	if magnitude < 1e-9 {
		return "0.00"
	}
	if magnitude >= 0.01 {
		return strconv.FormatFloat(value, 'f', 2, 64)
	}
	digits := 3 - int(math.Floor(math.Log10(magnitude)))
	text := strconv.FormatFloat(value, 'f', digits, 64)
	for strings.HasSuffix(text, "0") {
		text = strings.TrimSuffix(text, "0")
	}
	return text
}

// CompoundFeetInches renders whole feet plus remaining inches for the bare
// metric-length auto-conversion.
func CompoundFeetInches(feet float64) string {
	sign := ""
	if feet < 0 {
		sign = "-"
	}
	magnitude := math.Abs(feet)
	wholeFeet := math.Trunc(magnitude)
	inches := (magnitude - wholeFeet) * 12
	feetPart := ""
	if wholeFeet != 0 {
		unit := "feet"
		if wholeFeet == 1 {
			unit = "foot"
		}
		feetPart = sign + Display(wholeFeet) + " " + unit
	}
	inchText := Display(inches)
	inchUnit := "inches"
	if inchText == "1" {
		inchUnit = "inch"
	}
	inchPart := inchText + " " + inchUnit
	if feetPart == "" {
		return sign + inchPart
	}
	return feetPart + " " + inchPart
}

// Grouped inserts commas every three integer digits. Exponent-form strings pass
// through unchanged.
func Grouped(text string) string {
	if strings.ContainsAny(text, "eE") {
		return text
	}
	sign, unsigned := "", text
	if strings.HasPrefix(text, "-") {
		sign, unsigned = "-", text[1:]
	}
	intDigits, fraction := unsigned, ""
	if dot := strings.IndexByte(unsigned, '.'); dot >= 0 {
		intDigits, fraction = unsigned[:dot], unsigned[dot:]
	}
	if len(intDigits) <= 3 {
		return text
	}
	var builder strings.Builder
	builder.Grow(len(text) + len(intDigits)/3)
	builder.WriteString(sign)
	for index, digit := range intDigits {
		if index > 0 && (len(intDigits)-index)%3 == 0 {
			builder.WriteByte(',')
		}
		builder.WriteRune(digit)
	}
	builder.WriteString(fraction)
	return builder.String()
}

func formatG(value float64, precision int) string {
	if math.IsNaN(value) || math.IsInf(value, 0) {
		return strconv.FormatFloat(value, 'g', -1, 64)
	}
	sign := ""
	if math.Signbit(value) {
		sign = "-"
	}
	abs := math.Abs(value)
	if abs == 0 {
		return sign + "0"
	}
	exp := int(math.Floor(math.Log10(abs)))
	if abs >= pow10(exp+1) {
		exp++
	} else if abs < pow10(exp) {
		exp--
	}
	scaleExp := precision - 1 - exp
	digits := uint64(math.Round(abs * pow10(scaleExp)))
	overflowAt := uint64(math.Pow(10, float64(precision)))
	if digits >= overflowAt {
		digits /= 10
		exp++
	}
	digitText := strconv.FormatUint(digits, 10)
	for len(digitText) < precision {
		digitText = "0" + digitText
	}
	if len(digitText) > precision {
		digitText = digitText[:precision]
	}
	if exp < -4 || exp >= precision {
		head, rest := digitText[:1], strings.TrimRight(digitText[1:], "0")
		mantissa := head
		if rest != "" {
			mantissa += "." + rest
		}
		return sign + mantissa + "e" + signedExponent(exp)
	}
	if exp < 0 {
		var builder strings.Builder
		builder.WriteString("0.")
		builder.WriteString(strings.Repeat("0", -exp-1))
		builder.WriteString(strings.TrimRight(digitText, "0"))
		return sign + builder.String()
	}
	intLen := exp + 1
	var builder strings.Builder
	if intLen >= len(digitText) {
		builder.WriteString(digitText)
		builder.WriteString(strings.Repeat("0", intLen-len(digitText)))
	} else {
		builder.WriteString(digitText[:intLen])
		if fraction := strings.TrimRight(digitText[intLen:], "0"); fraction != "" {
			builder.WriteByte('.')
			builder.WriteString(fraction)
		}
	}
	return sign + builder.String()
}

func signedExponent(exp int) string {
	if exp < 0 {
		return "-" + padTwo(-exp)
	}
	return "+" + padTwo(exp)
}

func padTwo(value int) string {
	if value < 10 {
		return "0" + strconv.Itoa(value)
	}
	return strconv.Itoa(value)
}

func pow10(exp int) float64 {
	if exp >= 0 {
		return math.Pow(10, float64(exp))
	}
	return 1 / math.Pow(10, float64(-exp))
}
