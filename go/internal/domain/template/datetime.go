package template

import (
	"fmt"
	"strconv"
	"strings"
	"time"
)

var (
	enMonthsAbbr   = [...]string{"Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"}
	enMonthsFull   = [...]string{"January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"}
	frMonthsAbbr   = [...]string{"janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."}
	frMonthsFull   = [...]string{"janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"}
	enWeekdaysAbbr = [...]string{"Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"}
	enWeekdaysFull = [...]string{"Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"}
	frWeekdaysAbbr = [...]string{"dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."}
	frWeekdaysFull = [...]string{"dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"}
)

func formatDateTime(token DateTimeToken, ctx Context) string {
	when := ctx.Now
	if when.IsZero() {
		when = time.Now()
	}
	tz := when.Location()
	for _, offset := range token.Offsets {
		switch offset.Unit {
		case UnitMinute:
			when = when.Add(time.Duration(offset.Value) * time.Minute)
		case UnitHour:
			when = when.Add(time.Duration(offset.Value) * time.Hour)
		case UnitDay:
			when = when.AddDate(0, 0, offset.Value)
		case UnitMonth:
			when = when.AddDate(0, offset.Value, 0)
		case UnitYear:
			when = when.AddDate(offset.Value, 0, 0)
		}
	}
	when = when.In(tz)
	locale := token.Locale
	if locale == "" {
		locale = ctx.Locale
	}
	if token.Format != "" {
		return applyPattern(token.Format, when, locale)
	}
	switch token.Kind {
	case DateTimeTime:
		return shortTime(when, locale)
	case DateTimeBoth:
		return mediumDate(when, locale) + " " + datetimeJoiner(locale) + " " + shortTime(when, locale)
	case DateTimeWeekday:
		return weekdayName(when.Weekday(), locale, true)
	default:
		return mediumDate(when, locale)
	}
}

func language(locale string) string {
	cut := strings.IndexAny(locale, "-_")
	if cut < 0 {
		cut = len(locale)
	}
	head := locale[:cut]
	if len(head) > 2 {
		head = head[:2]
	}
	return head
}

func isFrench(locale string) bool {
	return strings.EqualFold(language(locale), "fr")
}

func datetimeJoiner(locale string) string {
	if isFrench(locale) {
		return "à"
	}
	return "at"
}

func mediumDate(when time.Time, locale string) string {
	month := monthName(int(when.Month()), locale, false)
	if isFrench(locale) {
		return fmt.Sprintf("%d %s %d", when.Day(), month, when.Year())
	}
	return fmt.Sprintf("%s %d, %d", month, when.Day(), when.Year())
}

func shortTime(when time.Time, locale string) string {
	if isFrench(locale) {
		return fmt.Sprintf("%02d:%02d", when.Hour(), when.Minute())
	}
	hour, ampm := hour12(when.Hour())
	return fmt.Sprintf("%d:%02d\u202f%s", hour, when.Minute(), ampm)
}

func hour12(hour int) (int, string) {
	ampm := "AM"
	if hour >= 12 {
		ampm = "PM"
	}
	value := hour % 12
	if value == 0 {
		value = 12
	}
	return value, ampm
}

func monthName(month int, locale string, full bool) string {
	index := month - 1
	if index < 0 || index > 11 {
		index = 0
	}
	if isFrench(locale) {
		if full {
			return frMonthsFull[index]
		}
		return frMonthsAbbr[index]
	}
	if full {
		return enMonthsFull[index]
	}
	return enMonthsAbbr[index]
}

func weekdayName(weekday time.Weekday, locale string, full bool) string {
	index := int(weekday) % 7
	if isFrench(locale) {
		if full {
			return frWeekdaysFull[index]
		}
		return frWeekdaysAbbr[index]
	}
	if full {
		return enWeekdaysFull[index]
	}
	return enWeekdaysAbbr[index]
}

func applyPattern(pattern string, when time.Time, locale string) string {
	var out strings.Builder
	runes := []rune(pattern)
	for i := 0; i < len(runes); {
		ch := runes[i]
		if ch == '\'' {
			i++
			if i < len(runes) && runes[i] == '\'' {
				out.WriteRune('\'')
				i++
				continue
			}
			for i < len(runes) {
				if runes[i] == '\'' {
					i++
					if i < len(runes) && runes[i] == '\'' {
						out.WriteRune('\'')
						i++
					} else {
						break
					}
				} else {
					out.WriteRune(runes[i])
					i++
				}
			}
			continue
		}
		if isASCIIAlpha(ch) {
			count := 1
			for i+count < len(runes) && runes[i+count] == ch {
				count++
			}
			out.WriteString(patternField(ch, count, when, locale))
			i += count
			continue
		}
		out.WriteRune(ch)
		i++
	}
	return out.String()
}

func isASCIIAlpha(ch rune) bool {
	return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z')
}

func patternField(letter rune, count int, when time.Time, locale string) string {
	switch letter {
	case 'y':
		year := when.Year()
		if year < 0 {
			year = -year
		}
		if count == 2 {
			return fmt.Sprintf("%02d", year%100)
		}
		width := max(count, 4)
		return fmt.Sprintf("%0*d", width, year)
	case 'M':
		switch count {
		case 1:
			return strconv.Itoa(int(when.Month()))
		case 2:
			return fmt.Sprintf("%02d", int(when.Month()))
		case 3:
			return monthName(int(when.Month()), locale, false)
		default:
			return monthName(int(when.Month()), locale, true)
		}
	case 'd':
		if count == 1 {
			return strconv.Itoa(when.Day())
		}
		return fmt.Sprintf("%02d", when.Day())
	case 'E':
		return weekdayName(when.Weekday(), locale, count >= 4)
	case 'H':
		if count == 1 {
			return strconv.Itoa(when.Hour())
		}
		return fmt.Sprintf("%02d", when.Hour())
	case 'h':
		hour, _ := hour12(when.Hour())
		if count == 1 {
			return strconv.Itoa(hour)
		}
		return fmt.Sprintf("%02d", hour)
	case 'm':
		if count == 1 {
			return strconv.Itoa(when.Minute())
		}
		return fmt.Sprintf("%02d", when.Minute())
	case 's':
		if count == 1 {
			return strconv.Itoa(when.Second())
		}
		return fmt.Sprintf("%02d", when.Second())
	case 'a':
		_, ampm := hour12(when.Hour())
		return ampm
	default:
		return strings.Repeat(string(letter), count)
	}
}
