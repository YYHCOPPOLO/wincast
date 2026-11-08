package ai

import (
	"errors"
	"net/url"
	"strings"
)

// EndpointError is shown beside the provider editor. Messages are stable so
// the UI can localize them.
type EndpointError string

const (
	ErrInvalidURL  EndpointError = "Invalid URL"
	ErrInsecureURL EndpointError = "Remote AI providers require an HTTPS base URL."
)

func (e EndpointError) Error() string { return string(e) }

// ParsedURL is the small destination identity needed for validation and key
// retention. Query and fragment are intentionally dropped.
type ParsedURL struct {
	Raw    string
	Scheme string
	Host   string
	Port   string
	Path   string
}

// ValidateURL accepts HTTPS remote endpoints and plain HTTP only on loopback.
func ValidateURL(raw string) (ParsedURL, error) {
	trimmed := strings.TrimSpace(raw)
	parsed, err := url.Parse(trimmed)
	if err != nil || parsed.Scheme == "" || parsed.Host == "" {
		return ParsedURL{}, ErrInvalidURL
	}
	scheme := strings.ToLower(parsed.Scheme)
	if scheme != "https" && scheme != "http" {
		return ParsedURL{}, ErrInvalidURL
	}
	if parsed.User != nil {
		return ParsedURL{}, ErrInvalidURL
	}
	host := parsed.Hostname()
	if host == "" {
		return ParsedURL{}, ErrInvalidURL
	}
	port := parsed.Port()
	if port == "" {
		if scheme == "https" {
			port = "443"
		} else {
			port = "80"
		}
	}
	path := parsed.EscapedPath()
	if path == "" {
		path = "/"
	}
	if scheme == "http" && !isLoopback(host) {
		return ParsedURL{}, ErrInsecureURL
	}
	return ParsedURL{Raw: trimmed, Scheme: scheme, Host: host, Port: port, Path: path}, nil
}

func isLoopback(host string) bool {
	switch strings.ToLower(host) {
	case "localhost", "127.0.0.1", "::1":
		return true
	}
	return false
}

// URLIsLoopback reports whether a saved endpoint needs no API key. It is used
// for local compatible servers only.
func URLIsLoopback(raw string) bool {
	parsed, err := ValidateURL(raw)
	return err == nil && isLoopback(parsed.Host)
}

// SameDestination compares scheme, host and port. Path changes are edits to
// the same provider and must not forget the key.
func SameDestination(a, b string) bool {
	left, leftErr := ValidateURL(a)
	right, rightErr := ValidateURL(b)
	if leftErr != nil || rightErr != nil {
		return strings.TrimSpace(a) == strings.TrimSpace(b)
	}
	return strings.EqualFold(left.Scheme, right.Scheme) &&
		strings.EqualFold(left.Host, right.Host) && left.Port == right.Port
}

// ErrNoKey is returned by the provider factory when a remote endpoint has no
// saved key.
var ErrNoKey = errors.New("Add an API key in Settings.")
