package calculator

import (
	"fmt"
	"math"
	"strconv"
	"strings"
)

type qKind uint8

const (
	qScalar qKind = iota
	qUnit
	qCurrency
)

type qValue struct {
	amount    float64
	kind      qKind
	unit      UnitDef
	currency  CurrencyDef
	isPercent bool
}

func (q qValue) effective() float64 {
	if q.isPercent {
		return q.amount / 100
	}
	return q.amount
}

type qBinary struct {
	op                byte
	bindingPower      int
	rightBindingPower int
	consumesToken     bool
}

type quantityParser struct {
	tokens         []token
	rates          *Rates
	position       int
	operationCount int
	dimensionCount int
	usedCurrency   bool
	currencyCodes  []string
	issue          string
}

func evalQuantity(tokens []token, query string, rates *Rates, region string, preserveStandaloneUnit bool) *Result {
	split := splitConversion(tokens)
	if split.target != "" && isSimpleConversionSource(split.expressionTokens) {
		return nil
	}
	parser := &quantityParser{tokens: split.expressionTokens, rates: rates}
	value, ok := parser.parse()
	if !ok {
		if parser.issue != "" {
			return errorResult(query, parser.issue)
		}
		return nil
	}
	if parser.dimensionCount == 0 {
		return nil
	}
	if parser.usedCurrency {
		if rates == nil {
			return errorResult(query, "Exchange rates unavailable — check your connection.")
		}
		for _, code := range parser.currencyCodes {
			if _, ok := rates.Rate(code); !ok {
				return errorResult(query, fmt.Sprintf("No exchange rate for %s.", code))
			}
		}
	}
	if split.target != "" {
		if output, ok := parser.converted(value, split.target); ok {
			return convertedResult(output, expressionText(split.expressionTokens))
		}
		if parser.issue != "" {
			return errorResult(query, parser.issue)
		}
		return nil
	}
	switch value.kind {
	case qScalar:
		if parser.operationCount == 0 {
			return nil
		}
		return valueResult(expressionText(split.expressionTokens), Display(value.effective()),
			CopyText(value.effective()), "Expression", "Result")
	case qUnit:
		if !preserveStandaloneUnit && parser.operationCount == 0 && parser.dimensionCount == 1 {
			if last := split.expressionTokens[len(split.expressionTokens)-1]; last.kind == tokIdent {
				if _, ok := LookupUnit(last.text); ok {
					return nil
				}
			}
		}
		if parser.operationCount == 0 && !preserveStandaloneUnit {
			return nil
		}
		return measurementResult(value.amount, value.unit, expressionText(split.expressionTokens))
	case qCurrency:
		if parser.operationCount != 0 {
			return currencyResult(value.amount, value.currency, expressionText(split.expressionTokens), "Expression")
		}
		expression := fmt.Sprintf("%s %s", Display(value.amount), value.currency.Code)
		if !preserveStandaloneUnit {
			if target, ok := regionTarget(region, value.currency); ok {
				if rates != nil {
					if output, ok := rates.Convert(value.amount, value.currency.Code, target.Code); ok {
						return currencyResult(output, target, expression, value.currency.Name)
					}
				}
			}
		}
		return currencyResult(value.amount, value.currency, expression, "Expression")
	}
	return nil
}

func regionTarget(region string, from CurrencyDef) (CurrencyDef, bool) {
	if region == "" {
		return CurrencyDef{}, false
	}
	target, ok := lookupCurrency(strings.ToLower(region))
	if !ok || target.Code == from.Code {
		return CurrencyDef{}, false
	}
	return target, true
}

func convertedResult(value qValue, expression string) *Result {
	switch value.kind {
	case qUnit:
		return measurementResult(value.amount, value.unit, expression)
	case qCurrency:
		return currencyResult(value.amount, value.currency, expression, "Expression")
	}
	return nil
}

func measurementResult(amount float64, unit UnitDef, expression string) *Result {
	return valueResult(expression,
		fmt.Sprintf("%s %s", Display(amount), unit.Symbol),
		fmt.Sprintf("%s %s", CopyText(amount), unit.Symbol),
		"Expression", unit.Name)
}

func currencyResult(amount float64, definition CurrencyDef, expression, sourceBadge string) *Result {
	formatted := Currency(amount)
	return valueResult(expression,
		fmt.Sprintf("%s %s", Grouped(formatted), definition.Code),
		fmt.Sprintf("%s %s", formatted, definition.Code),
		sourceBadge, definition.Name)
}

type splitConv struct {
	expressionTokens []token
	target           string
}

func splitConversion(tokens []token) splitConv {
	n := len(tokens)
	if n >= 3 && isConnector(tokens[n-2]) && tokens[n-1].kind == tokIdent {
		depth := 0
		for _, t := range tokens[:n-2] {
			if t.kind == tokOp {
				if t.op == '(' {
					depth++
				} else if t.op == ')' {
					depth--
				}
			}
		}
		if depth == 0 {
			return splitConv{expressionTokens: tokens[:n-2], target: tokens[n-1].text}
		}
	}
	return splitConv{expressionTokens: tokens}
}

func isSimpleConversionSource(tokens []token) bool {
	switch len(tokens) {
	case 1:
		return tokens[0].kind == tokIdent
	case 2:
		a, b := tokens[0], tokens[1]
		return (numberTokenKind(a) && b.kind == tokIdent) || (a.kind == tokIdent && numberTokenKind(b))
	}
	return false
}

func expressionText(tokens []token) string {
	var parts []string
	attachNext := true
	add := func(piece string, attached bool) {
		if (attachNext || attached) && len(parts) > 0 {
			parts[len(parts)-1] += piece
		} else {
			parts = append(parts, piece)
		}
		attachNext = false
	}
	for index := 0; index < len(tokens); index++ {
		t := tokens[index]
		if t.kind == tokIdent {
			if _, isUnit := LookupUnit(t.text); !isUnit {
				if definition, ok := lookupCurrency(t.text); ok && index+1 < len(tokens) {
					if amount, ok := numberValue(tokens[index+1]); ok {
						add(CopyText(amount), false)
						add(definition.Code, false)
						index++
						continue
					}
				}
			}
		}
		switch t.kind {
		case tokNumber, tokCompactNumber:
			add(CopyText(t.number), false)
		case tokIntLiteral:
			add(strconv.FormatUint(t.intValue, 10), false)
		case tokIdent:
			piece := t.text
			if unit, ok := LookupUnit(t.text); ok {
				piece = unit.Symbol
			} else if currency, ok := lookupCurrency(t.text); ok {
				piece = currency.Code
			}
			add(piece, false)
		case tokOp:
			switch t.op {
			case '(':
				add("(", false)
				attachNext = true
			case ')':
				add(")", true)
			case '%':
				add("%", true)
			case '!':
				add("!", true)
			case '*':
				add("×", false)
			case '/':
				add("÷", false)
			default:
				add(string(t.op), false)
				if t.op == '-' || t.op == '+' {
					attachNext = isSign(index, tokens)
				}
			}
		case tokArrow:
			add("→", false)
		}
	}
	return strings.Join(parts, " ")
}

func isSign(index int, tokens []token) bool {
	if index == 0 {
		return true
	}
	previous := tokens[index-1]
	if previous.kind != tokOp {
		return false
	}
	return previous.op != ')' && previous.op != '%' && previous.op != '!'
}

func (p *quantityParser) current() (token, bool) {
	if p.position >= len(p.tokens) {
		return token{}, false
	}
	return p.tokens[p.position], true
}

func (p *quantityParser) parse() (qValue, bool) {
	value, ok := p.parseExpression(0)
	if !ok || p.position != len(p.tokens) {
		return qValue{}, false
	}
	effective := value.effective()
	if math.IsInf(effective, 0) || math.IsNaN(effective) {
		return qValue{}, false
	}
	return value, true
}

func (p *quantityParser) parseExpression(minBP int) (qValue, bool) {
	left, ok := p.parseOperand()
	if !ok {
		return qValue{}, false
	}
	for {
		binary, ok := p.peekBinary(left)
		if !ok || binary.bindingPower < minBP {
			break
		}
		if binary.consumesToken {
			p.position++
		}
		p.operationCount++
		right, ok := p.parseExpression(binary.rightBindingPower)
		if !ok {
			return qValue{}, false
		}
		left, ok = p.apply(binary.op, left, right, !binary.consumesToken)
		if !ok {
			return qValue{}, false
		}
	}
	return left, true
}

func (p *quantityParser) peekBinary(left qValue) (qBinary, bool) {
	t, ok := p.current()
	if !ok {
		return qBinary{}, false
	}
	if t.kind == tokOp {
		switch t.op {
		case '+', '-':
			return qBinary{t.op, 10, 11, true}, true
		case '*', '/':
			return qBinary{t.op, 20, 21, true}, true
		case '^':
			return qBinary{t.op, 30, 30, true}, true
		case '(':
			return qBinary{'*', 20, 21, false}, true
		}
	}
	if t.kind == tokIdent && t.text == "of" {
		return qBinary{'*', 20, 21, true}, true
	}
	if left.kind != qScalar && p.startsQuantity() {
		return qBinary{'+', 10, 11, false}, true
	}
	return qBinary{}, false
}

func (p *quantityParser) apply(op byte, left, right qValue, implicit bool) (qValue, bool) {
	switch op {
	case '+', '-':
		return p.addOrSubtract(op, left, right, implicit)
	case '*':
		return p.multiply(left, right)
	case '/':
		return p.divide(left, right)
	case '^':
		if left.kind != qScalar || right.kind != qScalar {
			return qValue{}, false
		}
		output := math.Pow(left.effective(), right.effective())
		if math.IsInf(output, 0) || math.IsNaN(output) {
			return qValue{}, false
		}
		return qValue{amount: output, kind: qScalar}, true
	}
	return qValue{}, false
}

func (p *quantityParser) addOrSubtract(op byte, left, right qValue, implicit bool) (qValue, bool) {
	direction := 1.0
	if op == '-' {
		direction = -1
	}
	if right.isPercent {
		return qValue{amount: left.effective() * (1 + direction*right.amount/100), kind: left.kind}, true
	}
	switch {
	case left.kind == qScalar && right.kind == qScalar:
		return qValue{amount: left.effective() + direction*right.effective(), kind: qScalar}, true
	case left.kind == qUnit && right.kind == qUnit:
		if left.unit.Category != right.unit.Category {
			verb := "add"
			if op == '-' {
				verb = "subtract"
			}
			return p.fail(fmt.Sprintf("Cannot %s %s and %s.", verb,
				left.unit.Category.DisplayName(), right.unit.Category.DisplayName()))
		}
		if left.unit.Category == Temperature && left.unit.Symbol != right.unit.Symbol {
			return p.fail("Cannot combine temperatures with different units.")
		}
		if implicit {
			converted := ConvertUnit(right.amount, right.unit, left.unit)
			return qValue{amount: left.amount + direction*converted, kind: qUnit, unit: left.unit}, true
		}
		converted := ConvertUnit(left.amount, left.unit, right.unit)
		return qValue{amount: converted + direction*right.amount, kind: qUnit, unit: right.unit}, true
	case left.kind == qCurrency && right.kind == qCurrency:
		if implicit {
			converted, ok := p.convertedCurrency(right.amount, right.currency, left.currency)
			if !ok {
				return qValue{}, false
			}
			return qValue{amount: left.amount + direction*converted, kind: qCurrency, currency: left.currency}, true
		}
		converted, ok := p.convertedCurrency(left.amount, left.currency, right.currency)
		if !ok {
			return qValue{}, false
		}
		return qValue{amount: converted + direction*right.amount, kind: qCurrency, currency: right.currency}, true
	case left.kind == qUnit && right.kind == qCurrency:
		verb := "add"
		if op == '-' {
			verb = "subtract"
		}
		return p.fail(fmt.Sprintf("Cannot %s %s and %s.", verb,
			left.unit.Category.DisplayName(), currencyCategoryName))
	case left.kind == qCurrency && right.kind == qUnit:
		verb := "add"
		if op == '-' {
			verb = "subtract"
		}
		return p.fail(fmt.Sprintf("Cannot %s %s and %s.", verb,
			currencyCategoryName, right.unit.Category.DisplayName()))
	case (left.kind == qUnit || left.kind == qCurrency) && right.kind == qScalar:
		if implicit {
			return qValue{}, false
		}
		return qValue{amount: left.amount + direction*right.effective(), kind: left.kind,
			unit: left.unit, currency: left.currency}, true
	case left.kind == qScalar && (right.kind == qUnit || right.kind == qCurrency):
		if implicit {
			return qValue{}, false
		}
		return qValue{amount: left.effective() + direction*right.amount, kind: right.kind,
			unit: right.unit, currency: right.currency}, true
	}
	return qValue{}, false
}

func (p *quantityParser) multiply(left, right qValue) (qValue, bool) {
	switch {
	case left.kind == qScalar && right.kind == qScalar:
		return qValue{amount: left.effective() * right.effective(), kind: qScalar}, true
	case left.kind == qScalar:
		return qValue{amount: left.effective() * right.effective(), kind: right.kind,
			unit: right.unit, currency: right.currency}, true
	case right.kind == qScalar:
		return qValue{amount: left.effective() * right.effective(), kind: left.kind,
			unit: left.unit, currency: left.currency}, true
	}
	return p.fail("Multiplication of two unit values is not supported.")
}

func (p *quantityParser) divide(left, right qValue) (qValue, bool) {
	switch {
	case left.kind == qScalar && right.kind == qScalar:
		return p.finiteDivision(left.effective(), right.effective(), qValue{kind: qScalar})
	case (left.kind == qUnit || left.kind == qCurrency) && right.kind == qScalar:
		return p.finiteDivision(left.effective(), right.effective(), left)
	case left.kind == qScalar && (right.kind == qUnit || right.kind == qCurrency):
		return p.fail("Division by a unit value is not supported.")
	case left.kind == qUnit && right.kind == qUnit:
		if left.unit.Category != right.unit.Category {
			return p.fail(fmt.Sprintf("Cannot divide %s by %s.",
				left.unit.Category.DisplayName(), right.unit.Category.DisplayName()))
		}
		if left.unit.Category == Temperature {
			return p.fail("Division of temperature values is not supported.")
		}
		numerator := left.amount * left.unit.Factor
		denominator := right.amount * right.unit.Factor
		return p.finiteDivision(numerator, denominator, qValue{kind: qScalar})
	case left.kind == qCurrency && right.kind == qCurrency:
		denominator, ok := p.convertedCurrency(right.amount, right.currency, left.currency)
		if !ok {
			return qValue{}, false
		}
		return p.finiteDivision(left.amount, denominator, qValue{kind: qScalar})
	case left.kind == qUnit && right.kind == qCurrency:
		return p.fail(fmt.Sprintf("Cannot divide %s by %s.",
			left.unit.Category.DisplayName(), currencyCategoryName))
	case left.kind == qCurrency && right.kind == qUnit:
		return p.fail(fmt.Sprintf("Cannot divide %s by %s.",
			currencyCategoryName, right.unit.Category.DisplayName()))
	}
	return qValue{}, false
}

func (p *quantityParser) finiteDivision(numerator, denominator float64, kind qValue) (qValue, bool) {
	output := numerator / denominator
	if math.IsInf(output, 0) || math.IsNaN(output) {
		return qValue{}, false
	}
	kind.amount = output
	kind.isPercent = false
	return kind, true
}

func (p *quantityParser) parseOperand() (qValue, bool) {
	value, ok := p.parsePrefix()
	if !ok {
		return qValue{}, false
	}
	for {
		t, ok := p.current()
		if !ok {
			return value, true
		}
		if t.kind == tokIdent {
			if value.kind != qScalar || value.isPercent {
				return value, true
			}
			kind, ok := p.dimensionNamed(t.text)
			if !ok {
				return value, true
			}
			value.kind = kind.kind
			value.unit = kind.unit
			value.currency = kind.currency
			p.dimensionCount++
			p.position++
			continue
		}
		if t.kind == tokOp && t.op == '%' {
			if value.kind != qScalar || value.isPercent {
				return qValue{}, false
			}
			value.isPercent = true
			p.position++
			continue
		}
		if t.kind == tokOp && t.op == '!' {
			if value.kind != qScalar || value.isPercent {
				return qValue{}, false
			}
			fact, ok := factorial(value.amount)
			if !ok {
				return qValue{}, false
			}
			value.amount = fact
			p.position++
			continue
		}
		return value, true
	}
}

func (p *quantityParser) parsePrefix() (qValue, bool) {
	t, ok := p.current()
	if !ok {
		return qValue{}, false
	}
	switch t.kind {
	case tokNumber, tokCompactNumber:
		p.position++
		return qValue{amount: t.number, kind: qScalar}, true
	case tokIntLiteral:
		p.position++
		return qValue{amount: float64(t.intValue), kind: qScalar}, true
	case tokOp:
		switch t.op {
		case '-':
			p.position++
			value, ok := p.parseExpression(unaryBP)
			if !ok {
				return qValue{}, false
			}
			value.amount = -value.effective()
			value.isPercent = false
			return value, true
		case '+':
			p.position++
			return p.parseExpression(unaryBP)
		case '(':
			return p.parseGrouped()
		}
	case tokIdent:
		if _, ok := LookupUnit(t.text); ok {
			return qValue{}, false
		}
		definition, ok := lookupCurrency(t.text)
		if !ok {
			return qValue{}, false
		}
		amount, ok := p.numberAt(p.position + 1)
		if !ok {
			return qValue{}, false
		}
		p.position += 2
		p.recordCurrency(definition.Code)
		p.dimensionCount++
		return qValue{amount: amount, kind: qCurrency, currency: definition}, true
	}
	return qValue{}, false
}

func (p *quantityParser) parseGrouped() (qValue, bool) {
	close, ok := p.matchingParenthesis()
	if !ok {
		return qValue{}, false
	}
	p.position++
	target := p.groupedTarget(close)
	end := close
	if target != "" {
		end = close - 2
	}
	value, ok := p.parseGroupedValue(end)
	if !ok {
		return qValue{}, false
	}
	p.position = close + 1
	if target == "" {
		return value, true
	}
	p.operationCount++
	return p.converted(value, target)
}

func (p *quantityParser) parseGroupedValue(end int) (qValue, bool) {
	if end-p.position == 1 {
		t := p.tokens[p.position]
		if t.kind == tokIdent {
			if kind, ok := p.dimensionNamed(t.text); ok {
				p.position = end
				p.dimensionCount++
				kind.amount = 1
				return kind, true
			}
		}
	}
	value, ok := p.parseExpression(0)
	if !ok || p.position != end {
		return qValue{}, false
	}
	return value, true
}

func (p *quantityParser) matchingParenthesis() (int, bool) {
	if p.position >= len(p.tokens) || p.tokens[p.position].kind != tokOp || p.tokens[p.position].op != '(' {
		return 0, false
	}
	depth := 0
	for index := p.position; index < len(p.tokens); index++ {
		t := p.tokens[index]
		if t.kind != tokOp {
			continue
		}
		if t.op == '(' {
			depth++
		} else if t.op == ')' {
			depth--
			if depth == 0 {
				return index, true
			}
		}
	}
	return 0, false
}

func (p *quantityParser) groupedTarget(close int) string {
	if close-p.position < 3 || !isConnector(p.tokens[close-2]) {
		return ""
	}
	if p.tokens[close-1].kind != tokIdent {
		return ""
	}
	return p.tokens[close-1].text
}

func (p *quantityParser) converted(value qValue, targetName string) (qValue, bool) {
	switch value.kind {
	case qUnit:
		if to, ok := LookupUnit(targetName); ok {
			if value.unit.Category != to.Category {
				return p.fail(fmt.Sprintf("Cannot convert %s to %s.",
					value.unit.Category.DisplayName(), to.Category.DisplayName()))
			}
			output := ConvertUnit(value.amount, value.unit, to)
			if math.IsInf(output, 0) || math.IsNaN(output) {
				return qValue{}, false
			}
			return qValue{amount: output, kind: qUnit, unit: to}, true
		}
		if _, ok := lookupCurrency(targetName); ok {
			return p.fail(fmt.Sprintf("Cannot convert %s to %s.",
				value.unit.Category.DisplayName(), currencyCategoryName))
		}
	case qCurrency:
		if to, ok := lookupCurrency(targetName); ok {
			output, ok := p.convertedCurrency(value.amount, value.currency, to)
			if !ok {
				return qValue{}, false
			}
			return qValue{amount: output, kind: qCurrency, currency: to}, true
		}
		if to, ok := LookupUnit(targetName); ok {
			return p.fail(fmt.Sprintf("Cannot convert %s to %s.",
				currencyCategoryName, to.Category.DisplayName()))
		}
	}
	return qValue{}, false
}

func (p *quantityParser) dimensionNamed(name string) (qValue, bool) {
	if unit, ok := LookupUnit(name); ok {
		return qValue{kind: qUnit, unit: unit}, true
	}
	definition, ok := lookupCurrency(name)
	if !ok {
		return qValue{}, false
	}
	p.recordCurrency(definition.Code)
	return qValue{kind: qCurrency, currency: definition}, true
}

func (p *quantityParser) convertedCurrency(amount float64, from, to CurrencyDef) (float64, bool) {
	p.recordCurrency(from.Code)
	p.recordCurrency(to.Code)
	if p.rates == nil {
		p.issue = "Exchange rates unavailable — check your connection."
		return 0, false
	}
	if _, ok := p.rates.Rate(from.Code); !ok {
		p.issue = fmt.Sprintf("No exchange rate for %s.", from.Code)
		return 0, false
	}
	if _, ok := p.rates.Rate(to.Code); !ok {
		p.issue = fmt.Sprintf("No exchange rate for %s.", to.Code)
		return 0, false
	}
	output, ok := p.rates.Convert(amount, from.Code, to.Code)
	if !ok {
		p.issue = fmt.Sprintf("No exchange rate for %s.", to.Code)
	}
	return output, ok
}

func (p *quantityParser) recordCurrency(code string) {
	p.usedCurrency = true
	for _, existing := range p.currencyCodes {
		if existing == code {
			return
		}
	}
	p.currencyCodes = append(p.currencyCodes, code)
}

func (p *quantityParser) numberAt(index int) (float64, bool) {
	if index < 0 || index >= len(p.tokens) {
		return 0, false
	}
	return numberValue(p.tokens[index])
}

func (p *quantityParser) startsQuantity() bool {
	t, ok := p.current()
	if !ok {
		return false
	}
	switch t.kind {
	case tokNumber, tokCompactNumber, tokIntLiteral:
		return true
	case tokIdent:
		if _, isUnit := LookupUnit(t.text); isUnit {
			return false
		}
		if _, isCurrency := lookupCurrency(t.text); !isCurrency {
			return false
		}
		_, ok := p.numberAt(p.position + 1)
		return ok
	}
	return false
}

func (p *quantityParser) fail(message string) (qValue, bool) {
	p.issue = message
	return qValue{}, false
}
