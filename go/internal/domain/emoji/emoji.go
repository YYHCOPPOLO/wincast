package emoji

import "strings"

const CellDip = 56
const GridColumns = 8

type SkinTone uint8

const (
	ToneNone SkinTone = iota
	ToneLight
	ToneMediumLight
	ToneMedium
	ToneMediumDark
	ToneDark
)

var AllTones = []SkinTone{ToneNone, ToneLight, ToneMediumLight, ToneMedium, ToneMediumDark, ToneDark}

func ToneFromRaw(raw string) SkinTone {
	switch raw {
	case "light":
		return ToneLight
	case "medium-light":
		return ToneMediumLight
	case "medium":
		return ToneMedium
	case "medium-dark":
		return ToneMediumDark
	case "dark":
		return ToneDark
	default:
		return ToneNone
	}
}

func (t SkinTone) Raw() string {
	switch t {
	case ToneLight:
		return "light"
	case ToneMediumLight:
		return "medium-light"
	case ToneMedium:
		return "medium"
	case ToneMediumDark:
		return "medium-dark"
	case ToneDark:
		return "dark"
	default:
		return "none"
	}
}

func (t SkinTone) Label() string {
	switch t {
	case ToneLight:
		return "Light"
	case ToneMediumLight:
		return "Medium-Light"
	case ToneMedium:
		return "Medium"
	case ToneMediumDark:
		return "Medium-Dark"
	case ToneDark:
		return "Dark"
	default:
		return "Default"
	}
}

func (t SkinTone) Cycle() SkinTone {
	for index, tone := range AllTones {
		if tone == t {
			return AllTones[(index+1)%len(AllTones)]
		}
	}
	return ToneNone
}

func (t SkinTone) modifier() rune {
	switch t {
	case ToneLight:
		return '\U0001F3FB'
	case ToneMediumLight:
		return '\U0001F3FC'
	case ToneMedium:
		return '\U0001F3FD'
	case ToneMediumDark:
		return '\U0001F3FE'
	case ToneDark:
		return '\U0001F3FF'
	}
	return 0
}

type Emoji struct {
	Glyph     string
	Name      string
	Keywords  []string
	Group     string
	Skinnable bool
}

func Catalog() []Emoji {
	result := make([]Emoji, len(catalog))
	for i, item := range catalog {
		result[i] = Emoji{Glyph: item.glyph, Name: item.name, Keywords: item.keywords, Group: item.group, Skinnable: item.skinnable}
	}
	return result
}

func Search(query string, tone SkinTone) []Emoji {
	query = strings.ToLower(strings.TrimSpace(query))
	var result []Emoji
	for _, item := range catalog {
		if query != "" && !strings.Contains(strings.ToLower(item.name), query) &&
			!containsKeyword(item.keywords, query) {
			continue
		}
		glyph := item.glyph
		if item.skinnable {
			if modifier := tone.modifier(); modifier != 0 {
				glyph += string(modifier)
			}
		}
		result = append(result, Emoji{Glyph: glyph, Name: item.name, Keywords: item.keywords, Group: item.group, Skinnable: item.skinnable})
	}
	return result
}

func containsKeyword(keywords []string, query string) bool {
	for _, keyword := range keywords {
		if strings.Contains(keyword, query) {
			return true
		}
	}
	return false
}

type catalogEntry struct {
	glyph     string
	name      string
	keywords  []string
	group     string
	skinnable bool
}

var catalog = []catalogEntry{
	{"😀", "grinning face", []string{"smile", "happy"}, "Smileys", false},
	{"😃", "grinning face with big eyes", []string{"smile", "happy"}, "Smileys", false},
	{"😄", "grinning face with smiling eyes", []string{"smile", "happy"}, "Smileys", false},
	{"😁", "beaming face with smiling eyes", []string{"smile"}, "Smileys", false},
	{"😆", "grinning squinting face", []string{"laugh", "smile"}, "Smileys", false},
	{"😅", "grinning face with sweat", []string{"smile"}, "Smileys", false},
	{"🤣", "rolling on the floor laughing", []string{"laugh"}, "Smileys", false},
	{"😂", "face with tears of joy", []string{"laugh", "smile"}, "Smileys", false},
	{"🙂", "slightly smiling face", []string{"smile"}, "Smileys", false},
	{"😊", "smiling face with smiling eyes", []string{"smile", "happy"}, "Smileys", false},
	{"😇", "smiling face with halo", []string{"smile"}, "Smileys", false},
	{"☺", "smiling face", []string{"smile"}, "Smileys", false},
	{"😍", "smiling face with heart-eyes", []string{"love", "smile"}, "Smileys", false},
	{"🤩", "star-struck", []string{"smile"}, "Smileys", false},
	{"😘", "face blowing a kiss", []string{"love"}, "Smileys", false},
	{"👍", "thumbs up", []string{"yes", "ok"}, "People", true},
	{"👎", "thumbs down", []string{"no"}, "People", true},
	{"👏", "clapping hands", []string{"applause"}, "People", true},
	{"🙏", "folded hands", []string{"please", "thanks"}, "People", true},
	{"👋", "waving hand", []string{"hello", "hi"}, "People", true},
	{"🔥", "fire", []string{"hot"}, "Symbols", false},
	{"✨", "sparkles", []string{"star"}, "Symbols", false},
	{"⭐", "star", []string{"star"}, "Symbols", false},
	{"❤️", "red heart", []string{"love", "heart"}, "Symbols", false},
	{"💯", "hundred points", []string{"100"}, "Symbols", false},
	{"✅", "check mark button", []string{"check", "done"}, "Symbols", false},
	{"❌", "cross mark", []string{"x", "no"}, "Symbols", false},
	{"🎉", "party popper", []string{"party", "tada"}, "Activities", false},
	{"🚀", "rocket", []string{"ship"}, "Travel", false},
	{"💡", "light bulb", []string{"idea"}, "Objects", false},
	{"📝", "memo", []string{"note", "write"}, "Objects", false},
	{"🔗", "link", []string{"url"}, "Objects", false},
}
