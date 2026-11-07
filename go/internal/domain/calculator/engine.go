package calculator

import (
	"fmt"
	"math"
	"strconv"
	"strings"
)

type tokenKind uint8

const (
	tokNumber tokenKind = iota
	tokCompactNumber
	tokIntLiteral
	tokIdent
	tokOp
	tokArrow
)

type token struct {
	kind     tokenKind
	number   float64
	intValue uint64
	radix    int
	text     string
	op       byte
}

func numberToken(value float64) token  { return token{kind: tokNumber, number: value} }
func compactToken(value float64) token { return token{kind: tokCompactNumber, number: value} }
func intToken(value uint64, radix int) token {
	return token{kind: tokIntLiteral, intValue: value, radix: radix}
}
func identToken(name string) token { return token{kind: tokIdent, text: name} }
func opToken(op byte) token        { return token{kind: tokOp, op: op} }
func arrowToken() token            { return token{kind: tokArrow} }

// Evaluate is the pure calculator pipeline: datetime, arithmetic, base and
// unit conversions, quantities and injected currency rates.
func Evaluate(query string, now int64, rates *Rates, region string) *Result {
	query = strings.TrimSpace(query)
	if query == "" || len([]rune(query)) > 256 {
		return nil
	}
	if result := evaluateDateTime(query, now); result != nil {
		return result
	}
	tokens := tokenize(query)
	if len(tokens) == 0 {
		return nil
	}
	if result := partialResult(tokens, query, now, rates, region); result != nil {
		return result
	}
	if len(tokens) == 1 {
		switch tokens[0].kind {
		case tokIntLiteral:
			if tokens[0].radix != 10 {
				value := tokens[0].intValue
				display := Grouped(strconv.FormatUint(value, 10))
				return valueResult(query, display, strconv.FormatUint(value, 10), baseName(tokens[0].radix), "Decimal")
			}
		case tokCompactNumber:
			return valueResult(query, Display(tokens[0].number), CopyText(tokens[0].number), "Expression", "Result")
		}
		return nil
	}
	if result := baseConversion(tokens, query); result != nil {
		return result
	}
	if conversion, ok := parseUnitConversion(tokens); ok {
		return unitConversionResult(query, conversion)
	}
	if conversion, ok := parseUnitPair(tokens); ok {
		return unitConversionResult(query, conversion)
	}
	if quantity := evalQuantity(tokens, query, rates, region, false); quantity != nil {
		return quantity
	}
	if conversion, ok := parseCurrencyConversion(tokens, rates); ok {
		return currencyConversionResult(query, conversion)
	}
	if bare, ok := parseBareConversion(tokens); ok {
		var display, copyText string
		if bare.compound {
			text := CompoundFeetInches(bare.output)
			display, copyText = text, text
		} else {
			display = fmt.Sprintf("%s %s", Display(bare.output), bare.to.Symbol)
			copyText = fmt.Sprintf("%s %s", CopyText(bare.output), bare.to.Symbol)
		}
		return valueResult(fmt.Sprintf("%s %s", Display(bare.input), bare.from.Symbol),
			display, copyText, bare.from.Name, bare.to.Name)
	}
	if percent := evalPercent(tokens, query); percent != nil {
		return percent
	}
	hasDigit := false
	for _, r := range query {
		if r >= '0' && r <= '9' {
			hasDigit = true
			break
		}
	}
	hasE := strings.Contains(strings.ToLower(query), "e")
	if !hasDigit && !hasE && !strings.Contains(query, "π") {
		return nil
	}
	value, ok := parserEval(tokens)
	if !ok {
		return nil
	}
	return valueResult(prettyExpression(query), Display(value), CopyText(value), "Expression", "Result")
}

func partialResult(tokens []token, query string, now int64, rates *Rates, region string) *Result {
	if len(tokens) == 0 {
		return nil
	}
	operatorText, ok := partialOperatorText(tokens[len(tokens)-1])
	if !ok {
		return nil
	}
	prefix := tokens[:len(tokens)-1]
	if len(prefix) == 0 {
		return nil
	}
	if quantity := evalQuantity(prefix, tokenQuery(prefix), rates, region, true); quantity != nil {
		return quantity.withExpression(quantity.Expression + " " + operatorText)
	}
	if complete := Evaluate(tokenQuery(prefix), now, rates, region); complete != nil {
		return complete.withExpression(prettyExpression(query))
	}
	value, ok := parserEval(prefix)
	if !ok {
		return nil
	}
	return valueResult(prettyExpression(query), Display(value), CopyText(value), "Expression", "Result")
}

func partialOperatorText(t token) (string, bool) {
	if t.kind != tokOp {
		return "", false
	}
	switch t.op {
	case '*':
		return "×", true
	case '/':
		return "÷", true
	case '+', '-', '^':
		return string(t.op), true
	}
	return "", false
}

func tokenQuery(tokens []token) string {
	parts := make([]string, 0, len(tokens))
	for _, t := range tokens {
		switch t.kind {
		case tokNumber, tokCompactNumber:
			parts = append(parts, CopyText(t.number))
		case tokIntLiteral:
			prefix := ""
			switch t.radix {
			case 16:
				prefix = "0x"
			case 2:
				prefix = "0b"
			case 8:
				prefix = "0o"
			}
			parts = append(parts, prefix+formatIntRadix(t.intValue, t.radix))
		case tokIdent:
			parts = append(parts, t.text)
		case tokOp:
			parts = append(parts, string(t.op))
		case tokArrow:
			parts = append(parts, "->")
		}
	}
	return strings.Join(parts, " ")
}

func formatIntRadix(value uint64, radix int) string {
	return strconv.FormatUint(value, radix)
}

func prettyExpression(query string) string {
	fields := strings.Fields(query)
	text := strings.Join(fields, " ")
	text = strings.ReplaceAll(text, "*", "×")
	return strings.ReplaceAll(text, "/", "÷")
}

func baseConversion(tokens []token, query string) *Result {
	n := len(tokens)
	if n < 3 || !isConnector(tokens[n-2]) || tokens[n-1].kind != tokIdent {
		return nil
	}
	target := tokens[n-1].text
	valueTokens := tokens[:n-2]
	literalText := query
	if fields := strings.Fields(query); len(fields) > 0 {
		literalText = fields[0]
	}
	var source uint64
	sourceBadge, sourceText := "", ""
	if len(valueTokens) == 1 {
		t := valueTokens[0]
		if t.kind == tokIntLiteral {
			source, sourceBadge, sourceText = t.intValue, baseName(t.radix), literalText
		} else if value, ok := decimalLiteral(t); ok {
			if value >= 0 && math.Round(value) == value && value <= maxExactInteger {
				source, sourceBadge, sourceText = uint64(value), "Decimal", literalText
			} else {
				return nil
			}
		} else {
			return nil
		}
	} else if value, ok := parserEval(valueTokens); ok {
		if value >= 0 && math.Round(value) == value && value <= maxExactInteger {
			source = uint64(value)
			sourceBadge = "Decimal"
			sourceText = Grouped(strconv.FormatUint(source, 10))
		} else {
			return nil
		}
	} else {
		return nil
	}
	var output, targetBadge string
	switch target {
	case "hex", "hexadecimal":
		output, targetBadge = fmt.Sprintf("0x%X", source), "Hexadecimal"
	case "binary", "bin":
		output, targetBadge = fmt.Sprintf("0b%b", source), "Binary"
	case "octal", "oct":
		output, targetBadge = fmt.Sprintf("0o%o", source), "Octal"
	case "decimal", "dec":
		output, targetBadge = Grouped(strconv.FormatUint(source, 10)), "Decimal"
	default:
		return nil
	}
	copyText := strings.ReplaceAll(output, ",", "")
	return valueResult(sourceText, output, copyText, sourceBadge, targetBadge)
}

func baseName(radix int) string {
	switch radix {
	case 16:
		return "Hexadecimal"
	case 2:
		return "Binary"
	case 8:
		return "Octal"
	}
	return "Decimal"
}

func decimalLiteral(t token) (float64, bool) {
	if t.kind == tokNumber || t.kind == tokCompactNumber {
		return t.number, true
	}
	return 0, false
}

func isConnector(t token) bool {
	return t.kind == tokArrow || (t.kind == tokIdent && (t.text == "to" || t.text == "in"))
}

type unitConversion struct {
	from, to UnitDef
	input    float64
	output   float64
	mismatch bool
}

func parseUnitConversion(tokens []token) (unitConversion, bool) {
	n := len(tokens)
	if n < 3 || !isConnector(tokens[n-2]) || tokens[n-1].kind != tokIdent ||
		tokens[n-3].kind != tokIdent {
		return unitConversion{}, false
	}
	to, ok := LookupUnit(tokens[n-1].text)
	if !ok {
		return unitConversion{}, false
	}
	from, ok := LookupUnit(tokens[n-3].text)
	if !ok {
		return unitConversion{}, false
	}
	valueTokens := tokens[:n-3]
	input := 1.0
	if len(valueTokens) > 0 {
		var ok bool
		input, ok = parserEval(valueTokens)
		if !ok {
			return unitConversion{}, false
		}
	}
	if from.Category != to.Category {
		return unitConversion{from: from, to: to, mismatch: true}, true
	}
	output := ConvertUnit(input, from, to)
	if math.IsInf(output, 0) || math.IsNaN(output) {
		return unitConversion{}, false
	}
	return unitConversion{from: from, to: to, input: input, output: output}, true
}

func parseUnitPair(tokens []token) (unitConversion, bool) {
	if len(tokens) != 2 || tokens[0].kind != tokIdent || tokens[1].kind != tokIdent {
		return unitConversion{}, false
	}
	from, ok := LookupUnit(tokens[0].text)
	if !ok {
		return unitConversion{}, false
	}
	to, ok := LookupUnit(tokens[1].text)
	if !ok || from.Category != to.Category {
		return unitConversion{}, false
	}
	output := ConvertUnit(1, from, to)
	if math.IsInf(output, 0) || math.IsNaN(output) {
		return unitConversion{}, false
	}
	return unitConversion{from: from, to: to, input: 1, output: output}, true
}

func unitConversionResult(query string, conversion unitConversion) *Result {
	if conversion.mismatch {
		return errorResult(query, fmt.Sprintf("Cannot convert %s to %s.",
			conversion.from.Category.DisplayName(), conversion.to.Category.DisplayName()))
	}
	return valueResult(
		fmt.Sprintf("%s %s", Display(conversion.input), conversion.from.Symbol),
		fmt.Sprintf("%s %s", Display(conversion.output), conversion.to.Symbol),
		fmt.Sprintf("%s %s", CopyText(conversion.output), conversion.to.Symbol),
		conversion.from.Name, conversion.to.Name)
}

type bareConversion struct {
	input, output float64
	from, to      UnitDef
	compound      bool
}

func parseBareConversion(tokens []token) (bareConversion, bool) {
	if len(tokens) == 0 || tokens[len(tokens)-1].kind != tokIdent {
		return bareConversion{}, false
	}
	fromName := tokens[len(tokens)-1].text
	if fromName == "c" || fromName == "f" || fromName == "k" {
		return bareConversion{}, false
	}
	from, ok := LookupUnit(fromName)
	if !ok {
		return bareConversion{}, false
	}
	toName, compound, ok := AutoTarget(from.Symbol)
	if !ok {
		return bareConversion{}, false
	}
	to, ok := LookupUnit(toName)
	if !ok {
		return bareConversion{}, false
	}
	input, ok := parserEval(tokens[:len(tokens)-1])
	if !ok {
		return bareConversion{}, false
	}
	output := ConvertUnit(input, from, to)
	if math.IsInf(output, 0) || math.IsNaN(output) {
		return bareConversion{}, false
	}
	return bareConversion{input: input, output: output, from: from, to: to, compound: compound}, true
}

type currencyConversion struct {
	from, to      CurrencyDef
	input, output float64
	mismatch      bool
	noRate        string
	unavailable   bool
}

func parseCurrencyConversion(tokens []token, rates *Rates) (currencyConversion, bool) {
	tokens = amountFirst(tokens)
	n := len(tokens)
	if n < 3 || !isConnector(tokens[n-2]) || tokens[n-1].kind != tokIdent ||
		tokens[n-3].kind != tokIdent {
		return currencyConversion{}, false
	}
	fromName, toName := tokens[n-3].text, tokens[n-1].text
	from, fromOK := lookupCurrency(fromName)
	to, toOK := lookupCurrency(toName)
	switch {
	case !fromOK && !toOK:
		return currencyConversion{}, false
	case fromOK && !toOK:
		unit, ok := LookupUnit(toName)
		if !ok {
			return currencyConversion{}, false
		}
		return currencyConversion{mismatch: true, from: CurrencyDef{Name: "Currency"}, to: CurrencyDef{Name: unit.Category.DisplayName()}}, true
	case !fromOK && toOK:
		unit, ok := LookupUnit(fromName)
		if !ok {
			return currencyConversion{}, false
		}
		return currencyConversion{mismatch: true, from: CurrencyDef{Name: unit.Category.DisplayName()}, to: CurrencyDef{Name: "Currency"}}, true
	}
	valueTokens := tokens[:n-3]
	input := 1.0
	if len(valueTokens) > 0 {
		var ok bool
		input, ok = parserEval(valueTokens)
		if !ok {
			return currencyConversion{}, false
		}
	}
	if rates == nil {
		return currencyConversion{from: from, to: to, input: input, unavailable: true}, true
	}
	if _, ok := rates.Rate(from.Code); !ok {
		return currencyConversion{from: from, to: to, input: input, noRate: from.Code}, true
	}
	if _, ok := rates.Rate(to.Code); !ok {
		return currencyConversion{from: from, to: to, input: input, noRate: to.Code}, true
	}
	output, ok := rates.Convert(input, from.Code, to.Code)
	if !ok {
		return currencyConversion{from: from, to: to, input: input, noRate: to.Code}, true
	}
	return currencyConversion{from: from, to: to, input: input, output: output}, true
}

func amountFirst(tokens []token) []token {
	if len(tokens) >= 2 && tokens[0].kind == tokIdent {
		if _, ok := lookupCurrency(tokens[0].text); ok && numberTokenKind(tokens[1]) {
			reordered := append([]token(nil), tokens...)
			reordered[0], reordered[1] = reordered[1], reordered[0]
			return reordered
		}
	}
	return tokens
}

func numberTokenKind(t token) bool { return t.kind == tokNumber || t.kind == tokCompactNumber }
func numberValue(t token) (float64, bool) {
	if numberTokenKind(t) {
		return t.number, true
	}
	return 0, false
}

func currencyConversionResult(query string, conversion currencyConversion) *Result {
	switch {
	case conversion.mismatch:
		return errorResult(query, fmt.Sprintf("Cannot convert %s to %s.", conversion.from.Name, conversion.to.Name))
	case conversion.noRate != "":
		return errorResult(query, fmt.Sprintf("No exchange rate for %s.", conversion.noRate))
	case conversion.unavailable:
		return errorResult(query, "Exchange rates unavailable — check your connection.")
	}
	amount := Currency(conversion.output)
	return valueResult(
		fmt.Sprintf("%s %s", Display(conversion.input), conversion.from.Code),
		fmt.Sprintf("%s %s", Grouped(amount), conversion.to.Code),
		fmt.Sprintf("%s %s", amount, conversion.to.Code),
		conversion.from.Name, conversion.to.Name)
}

func evalPercent(tokens []token, query string) *Result {
	if result := parseOff(tokens, query); result != nil {
		return result
	}
	return parseAsPercentOf(tokens, query)
}

func parseOff(tokens []token, query string) *Result {
	off := -1
	for i, t := range tokens {
		if t.kind == tokIdent && t.text == "off" {
			off = i
			break
		}
	}
	if off < 2 || tokens[off-1].kind != tokOp || tokens[off-1].op != '%' {
		return nil
	}
	pct, ok1 := parserEval(tokens[:off-1])
	base, ok2 := parserEval(tokens[off+1:])
	if !ok1 || !ok2 {
		return nil
	}
	result := base * (1 - pct/100)
	if math.IsInf(result, 0) || math.IsNaN(result) {
		return nil
	}
	return percentCard(query, Display(result), CopyText(result))
}

func parseAsPercentOf(tokens []token, query string) *Result {
	as := -1
	for i, t := range tokens {
		if t.kind == tokIdent && t.text == "as" {
			as = i
			break
		}
	}
	if as < 0 || as+2 >= len(tokens) || tokens[as+1].kind != tokOp || tokens[as+1].op != '%' ||
		tokens[as+2].kind != tokIdent || tokens[as+2].text != "of" {
		return nil
	}
	x, ok1 := parserEval(tokens[:as])
	y, ok2 := parserEval(tokens[as+3:])
	if !ok1 || !ok2 || y == 0 {
		return nil
	}
	ratio := x / y * 100
	if math.IsInf(ratio, 0) || math.IsNaN(ratio) {
		return nil
	}
	return percentCard(query, Display(ratio)+"%", CopyText(ratio)+"%")
}

func percentCard(query, display, copyText string) *Result {
	return valueResult(strings.Join(strings.Fields(query), " "), display, copyText, "Expression", "Result")
}

func tokenize(input string) []token {
	chars := []rune(input)
	var tokens []token
	i := 0
	isDigit := func(r rune) bool { return r >= '0' && r <= '9' }
	for i < len(chars) {
		ch := chars[i]
		if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
			i++
			continue
		}
		if ch == '0' && i+2 < len(chars) {
			radix := 0
			switch chars[i+1] {
			case 'x', 'X':
				radix = 16
			case 'b', 'B':
				radix = 2
			case 'o', 'O':
				radix = 8
			}
			if radix != 0 {
				start := i + 2
				end := start
				for end < len(chars) && isHexDigit(chars[end]) {
					end++
				}
				if end > start {
					value, err := strconv.ParseUint(string(chars[start:end]), radix, 64)
					if err == nil {
						tokens = append(tokens, intToken(value, radix))
						i = end
						continue
					}
				}
			}
		}
		if isDigit(ch) || (ch == '.' && i+1 < len(chars) && isDigit(chars[i+1])) {
			var text strings.Builder
			seenDot := false
			for i < len(chars) {
				c := chars[i]
				if isDigit(c) {
					text.WriteRune(c)
				} else if c == ',' && i+1 < len(chars) && isDigit(chars[i+1]) {
					// grouping separator
				} else if c == '.' && !seenDot {
					seenDot = true
					text.WriteRune(c)
				} else {
					break
				}
				i++
			}
			shorthand := false
			if i < len(chars) && (chars[i] == 'e' || chars[i] == 'E') {
				digits := i + 1
				if digits < len(chars) && (chars[digits] == '+' || chars[digits] == '-') {
					digits++
				}
				end := digits
				for end < len(chars) && isDigit(chars[end]) {
					end++
				}
				if end > digits {
					text.WriteString(string(chars[i:end]))
					i = end
					shorthand = true
				}
			}
			value, err := strconv.ParseFloat(text.String(), 64)
			if err != nil || math.IsInf(value, 0) || math.IsNaN(value) {
				return nil
			}
			if i < len(chars) && (chars[i] == 'k' || chars[i] == 'K') && compactSuffix(chars, i) {
				tokens = append(tokens, compactToken(value*1000))
				i++
			} else if shorthand {
				tokens = append(tokens, compactToken(value))
			} else {
				tokens = append(tokens, numberToken(value))
			}
			continue
		}
		if isAlphabetic(ch) || ch == '°' {
			if isAlphabetic(ch) {
				letterEnd := i
				for letterEnd < len(chars) && isAlphabetic(chars[letterEnd]) {
					letterEnd++
				}
				if letterEnd < len(chars) && isDigit(chars[letterEnd]) {
					prefix := strings.ToLower(string(chars[i:letterEnd]))
					if _, unitOK := LookupUnit(prefix); !unitOK {
						if _, currencyOK := lookupCurrency(prefix); currencyOK {
							tokens = append(tokens, identToken(prefix))
							i = letterEnd
							continue
						}
					}
				}
			}
			var text strings.Builder
			for i < len(chars) {
				c := chars[i]
				if isAlphabetic(c) || c == '°' || isDigit(c) {
					text.WriteRune(c)
				} else if c == '²' {
					text.WriteRune('2')
				} else if c == '³' {
					text.WriteRune('3')
				} else {
					break
				}
				i++
			}
			tokens = append(tokens, identToken(strings.ToLower(text.String())))
			continue
		}
		if code := signCode(ch); code != "" {
			tokens = append(tokens, identToken(code))
			i++
			continue
		}
		if ch == '*' && i+1 < len(chars) && chars[i+1] == '*' {
			tokens = append(tokens, opToken('^'))
			i += 2
			continue
		}
		switch ch {
		case '+', '(', ')', '!', '%', '^':
			tokens = append(tokens, opToken(byte(ch)))
		case '*', '×':
			tokens = append(tokens, opToken('*'))
		case '/', '÷':
			tokens = append(tokens, opToken('/'))
		case '−':
			tokens = append(tokens, opToken('-'))
		case '-':
			if i+1 < len(chars) && chars[i+1] == '>' {
				tokens = append(tokens, arrowToken())
				i++
			} else {
				tokens = append(tokens, opToken('-'))
			}
		case '→':
			tokens = append(tokens, arrowToken())
		case '=':
			if i != len(chars)-1 {
				return nil
			}
		default:
			return nil
		}
		i++
	}
	return tokens
}

func isHexDigit(r rune) bool {
	return (r >= '0' && r <= '9') || (r >= 'a' && r <= 'f') || (r >= 'A' && r <= 'F')
}

func isAlphabetic(r rune) bool {
	return (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z')
}

func compactSuffix(chars []rune, index int) bool {
	next := index + 1
	if next >= len(chars) {
		return true
	}
	if isTemperatureConversion(chars, next) {
		return false
	}
	if !isAlphabetic(chars[next]) {
		return true
	}
	end := next
	for end < len(chars) && isAlphabetic(chars[end]) {
		end++
	}
	word := strings.ToLower(string(chars[next:end]))
	_, ok := lookupCurrency(word)
	return ok
}

func isTemperatureConversion(chars []rune, from int) bool {
	remainder := strings.ToLower(strings.TrimSpace(string(chars[from:])))
	for _, connector := range []string{"to", "in", "->", "→"} {
		if strings.HasPrefix(remainder, connector) {
			target := strings.TrimSpace(strings.TrimPrefix(remainder, connector))
			if unit, ok := LookupUnit(target); ok && unit.Category == Temperature {
				return true
			}
		}
	}
	return false
}

type scalar struct {
	value     float64
	isPercent bool
}

func (s scalar) effective() float64 {
	if s.isPercent {
		return s.value / 100
	}
	return s.value
}

type binaryOp struct {
	op                byte
	bindingPower      int
	rightBindingPower int
}

type parser struct {
	tokens []token
	pos    int
}

const unaryBP = 25

func parserEval(tokens []token) (float64, bool) {
	p := &parser{tokens: tokens}
	result, ok := p.parseExpression(0)
	if !ok || p.pos != len(tokens) {
		return 0, false
	}
	value := result.effective()
	if math.IsInf(value, 0) || math.IsNaN(value) {
		return 0, false
	}
	return value, true
}

func factorial(value float64) (float64, bool) {
	if value < 0 || math.Round(value) != value || value > 170 {
		return 0, false
	}
	result := 1.0
	for next := 2.0; next <= value; next++ {
		result *= next
	}
	return result, true
}

func constant(name string) (float64, bool) {
	switch name {
	case "pi", "π":
		return math.Pi, true
	case "e":
		return math.E, true
	}
	return 0, false
}

func function(name string) (func(float64) float64, bool) {
	switch name {
	case "sqrt":
		return math.Sqrt, true
	case "log":
		return math.Log10, true
	case "ln":
		return math.Log, true
	case "sin":
		return math.Sin, true
	case "cos":
		return math.Cos, true
	case "tan":
		return math.Tan, true
	case "abs":
		return math.Abs, true
	case "floor":
		return math.Floor, true
	case "ceil":
		return math.Ceil, true
	case "round":
		return math.Round, true
	}
	return nil, false
}

func (p *parser) current() (token, bool) {
	if p.pos >= len(p.tokens) {
		return token{}, false
	}
	return p.tokens[p.pos], true
}

func (p *parser) parseExpression(minBP int) (scalar, bool) {
	lhs, ok := p.parseOperand()
	if !ok {
		return scalar{}, false
	}
	for {
		if binary, ok := p.peekBinary(); ok && binary.bindingPower >= minBP {
			p.pos++
			rhs, ok := p.parseExpression(binary.rightBindingPower)
			if !ok {
				return scalar{}, false
			}
			lhs, ok = applyScalar(binary.op, lhs, rhs)
			if !ok {
				return scalar{}, false
			}
			continue
		}
		if p.impliesMultiplication() && 20 >= minBP {
			rhs, ok := p.parseExpression(21)
			if !ok {
				return scalar{}, false
			}
			lhs, ok = applyScalar('*', lhs, rhs)
			if !ok {
				return scalar{}, false
			}
			continue
		}
		break
	}
	return lhs, true
}

func (p *parser) impliesMultiplication() bool {
	t, ok := p.current()
	if !ok {
		return false
	}
	if t.kind == tokOp && t.op == '(' {
		return true
	}
	if t.kind == tokIdent {
		if _, ok := constant(t.text); ok {
			return true
		}
		_, ok := function(t.text)
		return ok
	}
	return false
}

func (p *parser) peekBinary() (binaryOp, bool) {
	t, ok := p.current()
	if !ok {
		return binaryOp{}, false
	}
	switch t.kind {
	case tokOp:
		switch t.op {
		case '+', '-':
			return binaryOp{t.op, 10, 11}, true
		case '*', '/':
			return binaryOp{t.op, 20, 21}, true
		case '^':
			return binaryOp{t.op, 30, 30}, true
		}
	case tokIdent:
		if t.text == "of" {
			return binaryOp{'*', 20, 21}, true
		}
		if t.text == "mod" {
			return binaryOp{'%', 20, 21}, true
		}
	}
	return binaryOp{}, false
}

func (p *parser) parseOperand() (scalar, bool) {
	value, ok := p.parsePrefix()
	if !ok {
		return scalar{}, false
	}
	for {
		t, ok := p.current()
		if !ok {
			break
		}
		switch {
		case t.kind == tokOp && t.op == '!':
			if value.isPercent {
				return scalar{}, false
			}
			fact, ok := factorial(value.value)
			if !ok {
				return scalar{}, false
			}
			value = scalar{value: fact}
		case t.kind == tokOp && t.op == '%':
			if value.isPercent {
				return scalar{}, false
			}
			value.isPercent = true
		case t.kind == tokIdent && t.text == "deg":
			if value.isPercent {
				return scalar{}, false
			}
			value = scalar{value: value.value * math.Pi / 180}
		default:
			return value, true
		}
		p.pos++
	}
	return value, true
}

func (p *parser) parsePrefix() (scalar, bool) {
	t, ok := p.current()
	if !ok {
		return scalar{}, false
	}
	switch t.kind {
	case tokNumber, tokCompactNumber:
		p.pos++
		return scalar{value: t.number}, true
	case tokIntLiteral:
		p.pos++
		return scalar{value: float64(t.intValue)}, true
	case tokOp:
		switch t.op {
		case '-':
			p.pos++
			operand, ok := p.parseExpression(unaryBP)
			if !ok {
				return scalar{}, false
			}
			return scalar{value: -operand.effective()}, true
		case '+':
			p.pos++
			return p.parseExpression(unaryBP)
		case '(':
			p.pos++
			inner, ok := p.parseExpression(0)
			if !ok {
				return scalar{}, false
			}
			close, ok := p.current()
			if !ok || close.kind != tokOp || close.op != ')' {
				return scalar{}, false
			}
			p.pos++
			return inner, true
		}
	case tokIdent:
		name := t.text
		if value, ok := constant(name); ok {
			p.pos++
			return scalar{value: value}, true
		}
		fn, ok := function(name)
		if !ok {
			return scalar{}, false
		}
		p.pos++
		var argument scalar
		if next, ok := p.current(); ok && next.kind == tokOp && next.op == '(' {
			p.pos++
			argument, ok = p.parseExpression(0)
			if !ok {
				return scalar{}, false
			}
			close, ok := p.current()
			if !ok || close.kind != tokOp || close.op != ')' {
				return scalar{}, false
			}
			p.pos++
		} else {
			argument, ok = p.parseOperand()
			if !ok {
				return scalar{}, false
			}
		}
		return scalar{value: fn(argument.effective())}, true
	}
	return scalar{}, false
}

func applyScalar(op byte, lhs, rhs scalar) (scalar, bool) {
	var result float64
	switch op {
	case '+':
		if rhs.isPercent {
			result = lhs.effective() * (1 + rhs.value/100)
		} else {
			result = lhs.effective() + rhs.effective()
		}
	case '-':
		if rhs.isPercent {
			result = lhs.effective() * (1 - rhs.value/100)
		} else {
			result = lhs.effective() - rhs.effective()
		}
	case '*':
		result = lhs.effective() * rhs.effective()
	case '/':
		result = lhs.effective() / rhs.effective()
	case '%':
		result = math.Mod(lhs.effective(), rhs.effective())
	case '^':
		result = math.Pow(lhs.effective(), rhs.effective())
	default:
		return scalar{}, false
	}
	if math.IsInf(result, 0) || math.IsNaN(result) {
		return scalar{}, false
	}
	return scalar{value: result}, true
}
