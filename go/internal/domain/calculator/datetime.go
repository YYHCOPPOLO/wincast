package calculator

import (
	"fmt"
	"strconv"
	"strings"
)

type momentBias uint8

const (
	biasFuture momentBias = iota
	biasPast
)

type moment struct {
	unix    int64
	hasTime bool
}

func evaluateDateTime(raw string, now int64) *Result {
	echo := strings.TrimSpace(raw)
	query := strings.ToLower(echo)
	if query == "" {
		return nil
	}
	hasUntil := strings.Contains(query, " till ") || strings.Contains(query, " until ") || strings.Contains(query, " til ")
	hasSince := strings.Contains(query, " since ")
	hasArith := strings.Contains(query, " + ") || strings.Contains(query, " - ")
	if !hasUntil && !hasSince && !hasArith {
		return nil
	}
	if hasUntil {
		if result := parseUntil(query, echo, now); result != nil {
			return result
		}
	}
	if hasSince {
		if result := parseSince(query, echo, now); result != nil {
			return result
		}
	}
	if hasArith {
		if result := parseDateArithmetic(query, echo, now); result != nil {
			return result
		}
	}
	return nil
}

func parseUntil(query, echo string, now int64) *Result {
	connector := ""
	for _, candidate := range []string{" until ", " till ", " til "} {
		if strings.Contains(query, candidate) {
			connector = candidate
			break
		}
	}
	if connector == "" {
		return nil
	}
	left, right, ok := splitTwo(query, connector)
	if !ok {
		return nil
	}
	unit, ok := durationUnit(left)
	if !ok {
		return nil
	}
	momentValue, ok := parseMoment(right, now, biasFuture)
	if !ok {
		return nil
	}
	reference := now
	if !unit.subDay {
		reference = startOfDay(now)
	}
	target := momentValue.unix
	if !unit.subDay {
		target = startOfDay(momentValue.unix)
	}
	value := durationValue(unit, reference, target)
	return durationCard(echo, value, unit, reference, target, now)
}

func parseSince(query, echo string, now int64) *Result {
	left, right, ok := splitTwo(query, " since ")
	if !ok {
		return nil
	}
	unit, ok := durationUnit(left)
	if !ok {
		return nil
	}
	momentValue, ok := parseMoment(right, now, biasPast)
	if !ok {
		return nil
	}
	reference := now
	if !unit.subDay {
		reference = startOfDay(now)
	}
	past := momentValue.unix
	if !unit.subDay {
		past = startOfDay(momentValue.unix)
	}
	value := durationValue(unit, past, reference)
	return durationCard(echo, value, unit, past, reference, now)
}

func parseDateArithmetic(query, echo string, now int64) *Result {
	plus := strings.Index(query, " + ")
	minus := strings.Index(query, " - ")
	opAt, op := -1, byte(0)
	switch {
	case plus >= 0 && minus >= 0 && plus < minus:
		opAt, op = plus, '+'
	case plus >= 0 && minus >= 0:
		opAt, op = minus, '-'
	case plus >= 0:
		opAt, op = plus, '+'
	case minus >= 0:
		opAt, op = minus, '-'
	default:
		return nil
	}
	left := query[:opAt]
	right := query[opAt+3:]
	base, ok := parseMoment(left, now, biasFuture)
	if !ok {
		return nil
	}
	if duration, ok := parseDurationPhrase(right); ok {
		if op == '-' && duration.count == -2147483648 {
			return nil
		}
		signed := duration.count
		if op == '-' {
			signed = -signed
		}
		result := base.unix + int64(signed)*duration.seconds
		hasTime := base.hasTime || duration.subDay
		display := momentString(result, hasTime, now)
		source := momentString(base.unix, base.hasTime, now)
		return valueResult(echo, display, display, source, "Result")
	}
	if op != '-' {
		return nil
	}
	if !hasAlphabetic(left) && !hasAlphabetic(right) {
		return nil
	}
	other, ok := parseMoment(right, now, biasFuture)
	if !ok {
		return nil
	}
	days := (startOfDay(base.unix) - startOfDay(other.unix)) / 86400
	word := "days"
	if absInt64(days) == 1 {
		word = "day"
	}
	display := fmt.Sprintf("%d %s", days, word)
	return valueResult(echo, display, display, dateString(base.unix, now), dateString(other.unix, now))
}

func hasAlphabetic(text string) bool {
	for _, r := range text {
		if r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' {
			return true
		}
	}
	return false
}

type durKind uint8

const (
	durSubSecond durKind = iota
	durDay
	durWeek
)

type durationDef struct {
	seconds  float64
	singular string
	plural   string
	kind     durKind
	subDay   bool
}

func durationUnit(phrase string) (durationDef, bool) {
	fields := strings.Fields(phrase)
	if len(fields) == 0 {
		return durationDef{}, false
	}
	return durationNamed(fields[len(fields)-1])
}

func durationNamed(name string) (durationDef, bool) {
	switch name {
	case "s", "sec", "secs", "second", "seconds":
		return durationDef{1, "second", "seconds", durSubSecond, true}, true
	case "min", "mins", "minute", "minutes":
		return durationDef{60, "minute", "minutes", durSubSecond, true}, true
	case "h", "hr", "hrs", "hour", "hours":
		return durationDef{3600, "hour", "hours", durSubSecond, true}, true
	case "d", "day", "days":
		return durationDef{86400, "day", "days", durDay, false}, true
	case "wk", "week", "weeks":
		return durationDef{604800, "week", "weeks", durWeek, false}, true
	}
	return durationDef{}, false
}

type durationPhrase struct {
	count   int64
	seconds int64
	subDay  bool
}

func parseDurationPhrase(phrase string) (durationPhrase, bool) {
	atoms := atomize(phrase)
	if len(atoms) != 2 {
		return durationPhrase{}, false
	}
	count, err := strconv.ParseInt(atoms[0], 10, 32)
	if err != nil {
		return durationPhrase{}, false
	}
	switch atoms[1] {
	case "s", "sec", "secs", "second", "seconds":
		return durationPhrase{count, 1, true}, true
	case "min", "mins", "minute", "minutes":
		return durationPhrase{count, 60, true}, true
	case "h", "hr", "hrs", "hour", "hours":
		return durationPhrase{count, 3600, true}, true
	case "d", "day", "days":
		return durationPhrase{count, 86400, false}, true
	case "wk", "week", "weeks":
		return durationPhrase{count * 7, 86400, false}, true
	}
	return durationPhrase{}, false
}

func durationCard(echo string, value float64, unit durationDef, sourceUnix, targetUnix, now int64) *Result {
	word := unit.plural
	if value == 1 {
		word = unit.singular
	}
	source := ""
	target := ""
	if unit.subDay {
		source = timeString(sourceUnix)
		target = timeString(targetUnix)
	} else {
		source = dateString(sourceUnix, now)
		target = dateString(targetUnix, now)
	}
	display := fmt.Sprintf("%s %s", Display(value), word)
	copyText := fmt.Sprintf("%s %s", CopyText(value), word)
	return valueResult(echo, display, copyText, source, target)
}

func durationValue(unit durationDef, from, to int64) float64 {
	switch unit.kind {
	case durDay:
		return float64((to - from) / 86400)
	case durWeek:
		return float64((to-from)/86400) / 7
	default:
		return float64(to-from) / unit.seconds
	}
}

func parseMoment(phrase string, now int64, bias momentBias) (moment, bool) {
	atoms := atomize(phrase)
	switch len(atoms) {
	case 1:
		return parseSingleMoment(atoms[0], now, bias)
	case 2:
		return parsePairMoment(atoms[0], atoms[1], now, bias)
	}
	return moment{}, false
}

func parseSingleMoment(atom string, now int64, bias momentBias) (moment, bool) {
	sod := startOfDay(now)
	switch atom {
	case "now":
		return moment{now, true}, true
	case "today":
		return moment{sod, false}, true
	case "tomorrow":
		return moment{sod + 86400, false}, true
	case "yesterday":
		return moment{sod - 86400, false}, true
	case "noon":
		return clockMoment(12, 0, now, bias)
	case "midnight":
		return clockMoment(0, 0, now, bias)
	}
	if weekday, ok := weekdayByName(atom); ok {
		return nextWeekday(weekday, false, bias == biasPast, now)
	}
	if month, ok := monthByName(atom); ok {
		return monthDayMoment(month, 1, now, bias)
	}
	return parseDateAtom(atom, now, bias)
}

func parsePairMoment(a, b string, now int64, bias momentBias) (moment, bool) {
	if month, ok := monthByName(b); ok {
		if day, err := strconv.ParseUint(a, 10, 32); err == nil {
			return monthDayMoment(month, uint32(day), now, bias)
		}
	}
	if month, ok := monthByName(a); ok {
		if day, err := strconv.ParseUint(b, 10, 32); err == nil {
			return monthDayMoment(month, uint32(day), now, bias)
		}
	}
	if b == "am" || b == "pm" {
		if hour, minute, ok := parseClock(a); ok {
			if hour < 1 || hour > 12 {
				return moment{}, false
			}
			adjusted := hour % 12
			if b == "pm" {
				adjusted = (hour % 12) + 12
			}
			return clockMoment(adjusted, minute, now, bias)
		}
	}
	if a == "next" || a == "last" {
		if weekday, ok := weekdayByName(b); ok {
			return nextWeekday(weekday, a == "next", a == "last", now)
		}
		if month, ok := monthByName(b); ok {
			pairBias := biasFuture
			if a == "last" {
				pairBias = biasPast
			}
			return monthDayMoment(month, 1, now, pairBias)
		}
	}
	return moment{}, false
}

func parseDateAtom(atom string, now int64, bias momentBias) (moment, bool) {
	if strings.Contains(atom, ":") {
		hour, minute, ok := parseClock(atom)
		if !ok {
			return moment{}, false
		}
		return clockMoment(hour, minute, now, bias)
	}
	if strings.Contains(atom, "-") {
		parts := strings.Split(atom, "-")
		if len(parts) != 3 {
			return moment{}, false
		}
		year, err := strconv.ParseInt(parts[0], 10, 32)
		if err != nil || year <= 31 {
			return moment{}, false
		}
		month, err1 := strconv.ParseUint(parts[1], 10, 32)
		day, err2 := strconv.ParseUint(parts[2], 10, 32)
		if err1 != nil || err2 != nil {
			return moment{}, false
		}
		unix, ok := makeDate(int32(year), uint32(month), uint32(day))
		return moment{unix, false}, ok
	}
	if strings.Contains(atom, "/") {
		parts := strings.Split(atom, "/")
		if len(parts) == 2 {
			month, err1 := strconv.ParseUint(parts[0], 10, 32)
			day, err2 := strconv.ParseUint(parts[1], 10, 32)
			if err1 != nil || err2 != nil {
				return moment{}, false
			}
			return monthDayMoment(uint32(month), uint32(day), now, bias)
		}
		if len(parts) == 3 {
			month, err1 := strconv.ParseUint(parts[0], 10, 32)
			day, err2 := strconv.ParseUint(parts[1], 10, 32)
			year, err3 := strconv.ParseInt(parts[2], 10, 32)
			if err1 != nil || err2 != nil || err3 != nil {
				return moment{}, false
			}
			unix, ok := makeDate(fullYear(int32(year)), uint32(month), uint32(day))
			return moment{unix, false}, ok
		}
	}
	return moment{}, false
}

func clockMoment(hour, minute uint32, now int64, bias momentBias) (moment, bool) {
	if hour > 23 || minute > 59 {
		return moment{}, false
	}
	date := startOfDay(now) + int64(hour)*3600 + int64(minute)*60
	if bias == biasFuture && date <= now {
		date += 86400
	}
	if bias == biasPast && date > now {
		date -= 86400
	}
	return moment{date, true}, true
}

func monthDayMoment(month uint32, day uint32, now int64, bias momentBias) (moment, bool) {
	year, _, _ := civilFromDays(floorDiv(now, 86400))
	thisYear, ok := makeDate(year, month, day)
	if !ok {
		return moment{}, false
	}
	sod := startOfDay(now)
	if bias == biasFuture {
		if thisYear >= sod {
			return moment{thisYear, false}, true
		}
		next, ok := makeDate(year+1, month, day)
		return moment{next, false}, ok
	}
	if thisYear <= sod {
		return moment{thisYear, false}, true
	}
	previous, ok := makeDate(year-1, month, day)
	return moment{previous, false}, ok
}

func nextWeekday(weekday uint32, offsetToFuture, past bool, now int64) (moment, bool) {
	sod := startOfDay(now)
	today := weekdayOf(sod)
	if past {
		back := (today + 7 - weekday) % 7
		if back == 0 {
			back = 7
		}
		return moment{sod - int64(back)*86400, false}, true
	}
	ahead := (weekday + 7 - today) % 7
	if ahead == 0 && offsetToFuture {
		ahead = 7
	}
	return moment{sod + int64(ahead)*86400, false}, true
}

func momentString(unix int64, hasTime bool, now int64) string {
	day := dateString(unix, now)
	if hasTime {
		return day + " at " + timeString(unix)
	}
	return day
}

func dateString(unix int64, now int64) string {
	year, month, day := civilFromDays(floorDiv(unix, 86400))
	nowYear, _, _ := civilFromDays(floorDiv(now, 86400))
	weekday := weekdays[(weekdayOf(unix)+6)%7]
	monthName := months[month-1]
	if year == nowYear {
		return fmt.Sprintf("%s, %d %s", weekday, day, monthName)
	}
	return fmt.Sprintf("%s, %d %s, %d", weekday, day, monthName, year)
}

func timeString(unix int64) string {
	sod := uint32(mod(unix, 86400))
	hour := sod / 3600
	minute := (sod % 3600) / 60
	ampm := "AM"
	if hour >= 12 {
		ampm = "PM"
	}
	h12 := hour % 12
	if h12 == 0 {
		h12 = 12
	}
	return fmt.Sprintf("%d:%02d %s", h12, minute, ampm)
}

func atomize(text string) []string {
	var atoms []string
	var current strings.Builder
	currentIsNumber := false
	flush := func() {
		if current.Len() > 0 {
			atoms = append(atoms, current.String())
			current.Reset()
		}
	}
	for _, r := range text {
		if r == ' ' {
			flush()
			continue
		}
		isNumeric := (r >= '0' && r <= '9') || r == ':' || r == '/' || r == '-' || r == '.'
		isLetter := (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z')
		if current.Len() == 0 {
			current.WriteRune(r)
			currentIsNumber = isNumeric && !isLetter
		} else if isLetter && currentIsNumber {
			flush()
			current.WriteRune(r)
			currentIsNumber = false
		} else if isNumeric && !isLetter && !currentIsNumber {
			flush()
			current.WriteRune(r)
			currentIsNumber = true
		} else {
			current.WriteRune(r)
		}
	}
	flush()
	return atoms
}

func parseClock(atom string) (uint32, uint32, bool) {
	if !strings.Contains(atom, ":") {
		hour, err := strconv.ParseUint(atom, 10, 32)
		if err != nil {
			return 0, 0, false
		}
		return uint32(hour), 0, true
	}
	parts := strings.Split(atom, ":")
	if len(parts) != 2 {
		return 0, 0, false
	}
	hour, err1 := strconv.ParseUint(parts[0], 10, 32)
	minute, err2 := strconv.ParseUint(parts[1], 10, 32)
	if err1 != nil || err2 != nil {
		return 0, 0, false
	}
	return uint32(hour), uint32(minute), true
}

func makeDate(year int32, month, day uint32) (int64, bool) {
	if month < 1 || month > 12 || day < 1 || day > 31 {
		return 0, false
	}
	if day > daysInMonth(year, month) {
		return 0, false
	}
	return daysFromCivil(year, month, day) * 86400, true
}

func daysInMonth(year int32, month uint32) uint32 {
	if month == 2 {
		if isLeap(year) {
			return 29
		}
		return 28
	}
	switch month {
	case 4, 6, 9, 11:
		return 30
	}
	return 31
}

func isLeap(year int32) bool {
	return year%4 == 0 && (year%100 != 0 || year%400 == 0)
}

func startOfDay(unix int64) int64 { return floorDiv(unix, 86400) * 86400 }

func weekdayOf(unix int64) uint32 {
	return uint32(mod(floorDiv(unix, 86400)+4, 7) + 1)
}

func fullYear(year int32) int32 {
	if year >= 100 {
		return year
	}
	if year <= 68 {
		return 2000 + year
	}
	return 1900 + year
}

func splitTwo(query, separator string) (string, string, bool) {
	parts := strings.Split(query, separator)
	if len(parts) != 2 {
		return "", "", false
	}
	return parts[0], parts[1], true
}

func monthByName(name string) (uint32, bool) {
	months := map[string]uint32{
		"january": 1, "jan": 1, "february": 2, "feb": 2, "march": 3, "mar": 3,
		"april": 4, "apr": 4, "may": 5, "june": 6, "jun": 6, "july": 7, "jul": 7,
		"august": 8, "aug": 8, "september": 9, "sep": 9, "sept": 9,
		"october": 10, "oct": 10, "november": 11, "nov": 11, "december": 12, "dec": 12,
	}
	month, ok := months[name]
	return month, ok
}

func weekdayByName(name string) (uint32, bool) {
	days := map[string]uint32{
		"sunday": 1, "sun": 1, "monday": 2, "mon": 2, "tuesday": 3, "tue": 3, "tues": 3,
		"wednesday": 4, "wed": 4, "thursday": 5, "thu": 5, "thurs": 5,
		"friday": 6, "fri": 6, "saturday": 7, "sat": 7,
	}
	day, ok := days[name]
	return day, ok
}

var weekdays = [7]string{"Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"}
var months = [12]string{"January", "February", "March", "April", "May", "June",
	"July", "August", "September", "October", "November", "December"}

func daysFromCivil(year int32, month, day uint32) int64 {
	y := int64(year)
	if month <= 2 {
		y--
	}
	era := y / 400
	if y < 0 {
		era = (y - 399) / 400
	}
	yoe := uint64(y - era*400)
	mp := month + 9
	if month > 2 {
		mp = month - 3
	}
	doy := (153*uint64(mp)+2)/5 + uint64(day) - 1
	doe := yoe*365 + yoe/4 - yoe/100 + doy
	return era*146097 + int64(doe) - 719468
}

func civilFromDays(z int64) (int32, uint32, uint32) {
	z += 719468
	era := z / 146097
	if z < 0 {
		era = (z - 146096) / 146097
	}
	doe := uint64(z - era*146097)
	yoe := (doe - doe/1460 + doe/36524 - doe/146096) / 365
	y := int64(yoe) + era*400
	doy := doe - (365*yoe + yoe/4 - yoe/100)
	mp := (5*doy + 2) / 153
	d := doy - (153*mp+2)/5 + 1
	m := mp + 3
	if mp >= 10 {
		m = mp - 9
	}
	if m <= 2 {
		y++
	}
	return int32(y), uint32(m), uint32(d)
}

func floorDiv(a, b int64) int64 {
	quotient := a / b
	if a%b != 0 && (a < 0) != (b < 0) {
		quotient--
	}
	return quotient
}

func mod(a, b int64) int64 {
	result := a % b
	if result < 0 {
		result += b
	}
	return result
}

func absInt64(value int64) int64 {
	if value < 0 {
		return -value
	}
	return value
}
