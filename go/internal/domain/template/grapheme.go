package template

import "unicode"

// GraphemeCount reports extended grapheme clusters, the unit caret movement
// uses when a cursor token repositions the caret after insertion.
func GraphemeCount(text string) int { return graphemeCount(text) }

// graphemeCount approximates UAX #29 extended grapheme clusters for caret
// movement: combining marks, joiners, variation selectors, skin-tone
// modifiers, keycaps, regional-indicator pairs and CRLF stay one cluster.
func graphemeCount(text string) int {
	count := 0
	previous := rune(0)
	previousWasZWJ := false
	regionalRun := 0
	havePrevious := false
	for _, current := range text {
		if !havePrevious {
			count++
			previous, havePrevious = current, true
			previousWasZWJ = current == 0x200D
			regionalRun = 0
			if isRegionalIndicator(current) {
				regionalRun = 1
			}
			continue
		}
		cluster := false
		switch {
		case previous == '\r' && current == '\n':
			cluster = true
		case previousWasZWJ:
			cluster = true
		case current == 0x200D:
			cluster = true
		case isCombining(current):
			cluster = true
		case isVariationSelector(current):
			cluster = true
		case isEmojiModifier(current):
			cluster = true
		case isKeycap(current):
			cluster = true
		case isRegionalIndicator(previous) && isRegionalIndicator(current) && regionalRun == 1:
			cluster = true
		}
		if !cluster {
			count++
		}
		if isRegionalIndicator(current) {
			if !cluster {
				regionalRun = 1
			} else {
				regionalRun++
			}
		} else {
			regionalRun = 0
		}
		previous = current
		previousWasZWJ = current == 0x200D
	}
	return count
}

func isCombining(ch rune) bool {
	return unicode.Is(unicode.Mn, ch) || unicode.Is(unicode.Mc, ch) || unicode.Is(unicode.Me, ch)
}

func isVariationSelector(ch rune) bool {
	return (ch >= 0xFE00 && ch <= 0xFE0F) || (ch >= 0xE0100 && ch <= 0xE01EF)
}

func isEmojiModifier(ch rune) bool {
	return ch >= 0x1F3FB && ch <= 0x1F3FF
}

func isKeycap(ch rune) bool {
	return ch == 0x20E3
}

func isRegionalIndicator(ch rune) bool {
	return ch >= 0x1F1E6 && ch <= 0x1F1FF
}
