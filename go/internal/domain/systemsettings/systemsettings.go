// Package systemsettings is the searchable catalog of Windows Settings panes.
// It is pure data: the launcher adds these entries and the settings window
// lists them, while the platform layer opens the URI.
package systemsettings

import "github.com/YYHCOPPOLO/wincast/go/internal/domain"

// Pane is one Windows Settings page.
type Pane struct {
	ID   string
	Name string
	URI  string
}

// All is the catalog, matching the panes the original application exposed.
var All = []Pane{
	{"app:ms-settings:display", "Display", "ms-settings:display"},
	{"app:ms-settings:bluetooth", "Bluetooth", "ms-settings:bluetooth"},
	{"app:ms-settings:appsfeatures", "Apps", "ms-settings:appsfeatures"},
	{"app:ms-settings:powersleep", "Power", "ms-settings:powersleep"},
	{"app:ms-settings:dateandtime", "Time", "ms-settings:dateandtime"},
	{"app:ms-settings:windowsupdate", "Update", "ms-settings:windowsupdate"},
	{"app:ms-settings:personalization", "Personalization", "ms-settings:personalization"},
	{"app:ms-settings:privacy", "Privacy", "ms-settings:privacy"},
	{"app:ms-settings:network", "Network", "ms-settings:network"},
	{"app:ms-settings:gaming-gamebar", "Gaming", "ms-settings:gaming-gamebar"},
	{"app:ms-settings:easeofaccess", "Accessibility", "ms-settings:easeofaccess"},
	{"app:ms-settings:accounts", "Accounts", "ms-settings:accounts"},
	{"app:ms-settings:about", "System", "ms-settings:about"},
}

// Entries converts the catalog to launcher rows.
func Entries() []domain.Application {
	entries := make([]domain.Application, 0, len(All))
	for _, pane := range All {
		entries = append(entries, domain.Application{
			ID: pane.ID, Name: pane.Name, Path: pane.URI, Kind: domain.KindSystemSettings,
		})
	}
	return entries
}

// URI resolves an entry ID to its Settings URI.
func URI(id string) (string, bool) {
	for _, pane := range All {
		if pane.ID == id {
			return pane.URI, true
		}
	}
	return "", false
}
