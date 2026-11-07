// Package quicklink holds the pure quicklink model: ordering, destination
// classification and template expansion. It never touches the filesystem.
package quicklink

import (
	"net/url"
	"sort"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/template"
)

type DestinationKind uint8

const (
	DestinationNone DestinationKind = iota
	DestinationPath
	DestinationWeb
	DestinationNetwork
	DestinationDeeplink
)

// Link is one authored quicklink. Pinned links precede the rest in the order
// they were pinned; the rest sort by name.
type Link struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Destination string `json:"destination"`
	Pinned      bool   `json:"pinned,omitempty"`
	ShowInRoot  bool   `json:"showInRoot"`
	PinOrder    uint64 `json:"pinOrder,omitempty"`
	Hotkey      string `json:"hotkey,omitempty"`
}

// EntryID is the stable launcher identity for a quicklink.
func EntryID(id string) string { return "quicklink:" + id }

// IDFromEntry returns the quicklink id from a launcher entry identity.
func IDFromEntry(entryID string) (string, bool) {
	return strings.CutPrefix(entryID, "quicklink:")
}

// Sort orders links by pin state, pin order, then case-insensitive name.
func Sort(links []Link) {
	sort.SliceStable(links, func(i, j int) bool { return Precedes(links[i], links[j]) })
}

// Precedes is the single display order shared by the store and the launcher.
func Precedes(a, b Link) bool {
	switch {
	case a.Pinned && !b.Pinned:
		return true
	case !a.Pinned && b.Pinned:
		return false
	case a.Pinned && b.Pinned:
		if a.PinOrder != b.PinOrder {
			return a.PinOrder < b.PinOrder
		}
	default:
		left, right := strings.ToLower(a.Name), strings.ToLower(b.Name)
		if left != right {
			return left < right
		}
		return a.ID < b.ID
	}
	left, right := strings.ToLower(a.Name), strings.ToLower(b.Name)
	if left != right {
		return left < right
	}
	return a.ID < b.ID
}

// Detect classifies a destination from its shape alone. A one-letter scheme is
// a drive letter; a scheme whose remainder is digits is a host:port.
func Detect(raw string) DestinationKind {
	text := strings.TrimSpace(raw)
	if text == "" {
		return DestinationNone
	}
	if strings.HasPrefix(text, "~/") || strings.HasPrefix(text, "/") || strings.HasPrefix(text, "file:") {
		return DestinationPath
	}
	if looksLikeWindowsPath(text) {
		return DestinationPath
	}
	if scheme, rest, found := strings.Cut(text, ":"); found {
		scheme = strings.ToLower(scheme)
		if len(scheme) == 1 && isASCIIAlpha(scheme[0]) {
			return DestinationPath
		}
		if allDigits(rest) {
			return DestinationNone
		}
		switch scheme {
		case "http", "https":
			return DestinationWeb
		case "smb", "afp", "nfs", "ftp", "sftp", "ftps":
			return DestinationNetwork
		case "file":
			return DestinationPath
		default:
			return DestinationDeeplink
		}
	}
	if looksLikeHost(text) {
		return DestinationWeb
	}
	return DestinationNone
}

func looksLikeWindowsPath(text string) bool {
	if len(text) >= 3 && isASCIIAlpha(text[0]) && text[1] == ':' && (text[2] == '\\' || text[2] == '/') {
		return true
	}
	return strings.HasPrefix(text, `\\`)
}

func looksLikeHost(text string) bool {
	host := text
	if cut := strings.IndexAny(host, "/?#"); cut >= 0 {
		host = host[:cut]
	}
	dot := strings.LastIndexByte(host, '.')
	if dot <= 0 || dot+1 >= len(host) {
		return false
	}
	tld := host[dot+1:]
	return len(tld) >= 2 && isASCIIAlpha(tld[0])
}

func allDigits(text string) bool {
	if text == "" {
		return false
	}
	for i := 0; i < len(text); i++ {
		if text[i] < '0' || text[i] > '9' {
			return false
		}
	}
	return true
}

func isASCIIAlpha(ch byte) bool {
	return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z')
}

// UsesURLEncoding decides whether substituted values are percent-encoded. The
// answer comes from the link prefix because it must be known before values
// are resolved.
func UsesURLEncoding(destination string) bool {
	kind := Detect(destination)
	return kind != DestinationNone && kind != DestinationPath
}

// Expand resolves placeholders and reports missing arguments.
func Expand(destination string, ctx template.Context, args map[string]string) template.Output {
	return template.ExpandDestination(destination, ctx, args, UsesURLEncoding(destination))
}

// OpenKind is how the platform should open an expanded destination.
type OpenKind uint8

const (
	OpenExternal OpenKind = iota
	OpenPath
)

// Resolve turns an expanded destination into the platform action.
func Resolve(expanded string) (OpenKind, string) {
	text := strings.TrimSpace(expanded)
	if strings.HasPrefix(text, "file:") {
		path := strings.TrimPrefix(text, "file:")
		path = strings.TrimPrefix(path, "//")
		if len(path) >= 3 && path[0] == '/' && isASCIIAlpha(path[1]) && path[2] == ':' {
			path = path[1:]
		}
		if decoded, err := url.PathUnescape(path); err == nil {
			path = decoded
		}
		return OpenPath, path
	}
	if Detect(text) == DestinationPath {
		return OpenPath, text
	}
	if Detect(text) == DestinationWeb && !strings.Contains(text, "://") && !strings.HasPrefix(text, "http") {
		return OpenExternal, "https://" + text
	}
	return OpenExternal, text
}
