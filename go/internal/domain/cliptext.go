package domain

import "strings"

type ClipboardTextForm uint8

const (
	ClipTextPlain ClipboardTextForm = iota
	ClipTextLink
	ClipTextEmail
)

const clipboardDetectionLimit = 2048

var commonTLDs = map[string]bool{
	"com": true, "org": true, "net": true, "edu": true, "gov": true, "io": true,
	"co": true, "ai": true, "app": true, "dev": true, "me": true, "info": true,
	"biz": true, "xyz": true, "tv": true, "ly": true, "gg": true, "to": true,
	"uk": true, "us": true, "eu": true, "de": true, "fr": true, "es": true,
	"it": true, "nl": true, "se": true, "no": true, "fi": true, "dk": true,
	"ch": true, "at": true, "be": true, "ie": true, "cz": true, "ru": true,
	"ua": true, "tr": true, "cn": true, "jp": true, "kr": true, "hk": true,
	"sg": true, "au": true, "nz": true, "ca": true, "mx": true, "br": true,
	"ar": true, "za": true,
}

// ClassifyClipboardText is the cheap, never-persisted filter used by the
// clipboard palette. It intentionally only classifies short, single tokens.
func ClassifyClipboardText(text string) ClipboardTextForm {
	if len(text) > clipboardDetectionLimit {
		return ClipTextPlain
	}
	token := strings.TrimSpace(text)
	if token == "" || strings.ContainsAny(token, " \t\r\n") {
		return ClipTextPlain
	}
	if form, ok := schemeForm(token); ok {
		return form
	}
	if isAddress(token) {
		return ClipTextEmail
	}
	if isBareDomain(token) {
		return ClipTextLink
	}
	return ClipTextPlain
}

func schemeForm(token string) (ClipboardTextForm, bool) {
	if len(token) >= 7 && strings.EqualFold(token[:7], "mailto:") {
		return ClipTextEmail, true
	}
	separator := strings.Index(token, "://")
	if separator < 0 {
		return ClipTextPlain, false
	}
	scheme := token[:separator]
	if scheme == "" {
		return ClipTextPlain, false
	}
	for _, r := range scheme {
		if !isASCIIAlpha(r) && r != '+' && r != '-' && r != '.' {
			return ClipTextPlain, false
		}
	}
	return ClipTextLink, true
}

func isASCIIAlpha(r rune) bool {
	return (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z')
}

func isAddress(token string) bool {
	parts := strings.Split(token, "@")
	if len(parts) != 2 || parts[0] == "" {
		return false
	}
	return topLevelLabel(parts[1]) != ""
}

func isBareDomain(token string) bool {
	hostEnd := len(token)
	if index := strings.IndexAny(token, "/?#:"); index >= 0 {
		hostEnd = index
	}
	host := token[:hostEnd]
	if len(host) >= 4 && strings.EqualFold(host[:4], "www.") {
		return true
	}
	if strings.ToLower(host) != host {
		return false
	}
	tld := topLevelLabel(host)
	return tld != "" && commonTLDs[tld]
}

func topLevelLabel(host string) string {
	labels := strings.Split(host, ".")
	if len(labels) < 2 {
		return ""
	}
	for _, label := range labels {
		if !isHostLabel(label) {
			return ""
		}
	}
	tld := labels[len(labels)-1]
	if len(tld) < 2 {
		return ""
	}
	for _, r := range tld {
		if !isASCIIAlpha(r) {
			return ""
		}
	}
	return tld
}

func isHostLabel(label string) bool {
	if label == "" {
		return false
	}
	for _, r := range label {
		if !isASCIIAlpha(r) && !(r >= '0' && r <= '9') && r != '-' {
			return false
		}
	}
	return true
}
