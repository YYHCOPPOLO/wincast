package extensions

import (
	"archive/zip"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// Listing is one registry result.
type Listing struct {
	ID           string
	Name         string
	Title        string
	Summary      string
	Author       string
	LightIconURL string
	DarkIconURL  string
	CommandCount int
	Downloads    int
	Platforms    []string
	Registry     string
	Source       ListingSource
}

// ListingSource is either a prebuilt store zip or a GitHub source folder.
type ListingSource struct {
	Kind       string // "zip", "github"
	URL        string
	Owner      string
	Repository string
	Path       string
	Ref        string
}

// Registry is a place extensions are searched for and fetched from.
type Registry struct {
	ID         string
	Name       string
	Kind       string // "store", "github"
	Owner      string
	Repository string
	Path       string
	Ref        string
	Enabled    bool
}

// DefaultRegistries are the two sources the original app ships with.
func DefaultRegistries() []Registry {
	return []Registry{
		{ID: "raycast-store", Name: "Raycast Store", Kind: "store", Enabled: true},
		{ID: "raycast-extensions", Name: "raycast/extensions", Kind: "github",
			Owner: "raycast", Repository: "extensions", Path: "extensions", Ref: "main", Enabled: true},
	}
}

type storePayload struct {
	Data []storeEntry `json:"data"`
}

type storeEntry struct {
	ID            string `json:"id"`
	Name          string `json:"name"`
	Title         string `json:"title"`
	Description   string `json:"description"`
	DownloadURL   string `json:"download_url"`
	DownloadCount int    `json:"download_count"`
	Status        string `json:"status"`
	Author        struct {
		Name   string `json:"name"`
		Handle string `json:"handle"`
	} `json:"author"`
	Icons struct {
		Light string `json:"light"`
		Dark  string `json:"dark"`
	} `json:"icons"`
	Commands []struct {
		Name string `json:"name"`
	} `json:"commands"`
	Platforms []string `json:"platforms"`
}

func httpGet(target string) ([]byte, error) {
	request, err := http.NewRequest(http.MethodGet, target, nil)
	if err != nil {
		return nil, err
	}
	request.Header.Set("User-Agent", "Wincast")
	request.Header.Set("Accept", "application/vnd.github+json")
	client := &http.Client{Timeout: 30 * time.Second}
	response, err := client.Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	data, err := io.ReadAll(response.Body)
	if err != nil {
		return nil, err
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		var message struct {
			Message string `json:"message"`
		}
		if json.Unmarshal(data, &message) == nil && message.Message != "" {
			return nil, errors.New(message.Message)
		}
		return nil, fmt.Errorf("HTTP %d", response.StatusCode)
	}
	return data, nil
}

// SearchStore queries Raycast's own search endpoint, the one its website uses.
func SearchStore(query string) ([]Listing, error) {
	components, _ := url.Parse("https://www.raycast.com/frontend_api/extensions/search")
	values := components.Query()
	values.Set("q", query)
	values.Set("page", "1")
	components.RawQuery = values.Encode()
	data, err := httpGet(components.String())
	if err != nil {
		return nil, err
	}
	var payload storePayload
	if err := json.Unmarshal(data, &payload); err != nil {
		return nil, fmt.Errorf("read the Raycast Store response: %w", err)
	}
	listings := make([]Listing, 0, len(payload.Data))
	for _, entry := range payload.Data {
		if entry.Status != "" && entry.Status != "active" {
			continue
		}
		if entry.DownloadURL == "" {
			continue
		}
		if !storeSupportsWindows(entry.Platforms) {
			continue
		}
		author := entry.Author.Name
		if author == "" {
			author = entry.Author.Handle
		}
		title := entry.Title
		if title == "" {
			title = entry.Name
		}
		listings = append(listings, Listing{
			ID: entry.ID, Name: entry.Name, Title: title, Summary: entry.Description,
			Author: author, LightIconURL: entry.Icons.Light, DarkIconURL: entry.Icons.Dark,
			CommandCount: len(entry.Commands), Downloads: entry.DownloadCount, Platforms: entry.Platforms,
			Registry: "Raycast Store", Source: ListingSource{Kind: "zip", URL: entry.DownloadURL},
		})
	}
	return listings, nil
}

// storeSupportsWindows accepts an absent platform list; when the Store names
// platforms, Windows must be among them.
func storeSupportsWindows(platforms []string) bool {
	if len(platforms) == 0 {
		return true
	}
	for _, platform := range platforms {
		if strings.EqualFold(platform, "windows") {
			return true
		}
	}
	return false
}

// SearchRegistries searches every enabled source. One failing registry does not
// take the others' results with it.
func (m *Manager) SearchRegistries(query string) ([]Listing, []string) {
	query = strings.TrimSpace(query)
	if query == "" {
		return nil, nil
	}
	listings, err := SearchStore(query)
	failures := make([]string, 0)
	if err != nil {
		failures = append(failures, "Raycast Store: "+err.Error())
	}
	github, githubErr := searchGitHub(query)
	if githubErr != nil {
		failures = append(failures, "raycast/extensions: "+githubErr.Error())
	}
	listings = append(listings, github...)
	return listings, failures
}

// InstallListing downloads a prebuilt store bundle, unpacks it and installs
// the result. GitHub source listings are built with ray when the toolchain is
// available.
func (m *Manager) InstallListing(listing Listing) (*Installed, error) {
	m.installMu.Lock()
	defer m.installMu.Unlock()
	workspace, err := os.MkdirTemp("", "wincast-install-")
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(workspace)
	var prepared string
	switch listing.Source.Kind {
	case "zip":
		prepared, err = m.prepareZip(listing.Source.URL, workspace)
	case "github":
		prepared, err = m.prepareGitHub(listing.Source, workspace)
	default:
		return nil, fmt.Errorf("Unknown extension source %q.", listing.Source.Kind)
	}
	if err != nil {
		return nil, err
	}
	installed, err := m.catalog.Install(prepared)
	if err != nil {
		return nil, err
	}
	m.Refresh()
	return installed, nil
}

func (m *Manager) prepareZip(target, workspace string) (string, error) {
	data, err := httpGet(target)
	if err != nil {
		return "", err
	}
	archive := filepath.Join(workspace, "extension.zip")
	if err := os.WriteFile(archive, data, 0o600); err != nil {
		return "", err
	}
	expanded := filepath.Join(workspace, "expanded")
	if err := os.MkdirAll(expanded, 0o700); err != nil {
		return "", err
	}
	if err := unzip(archive, expanded); err != nil {
		return "", err
	}
	return manifestRoot(expanded)
}

func manifestRoot(directory string) (string, error) {
	if _, err := os.Stat(filepath.Join(directory, "package.json")); err == nil {
		return directory, nil
	}
	entries, err := os.ReadDir(directory)
	if err != nil {
		return "", err
	}
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		candidate := filepath.Join(directory, entry.Name())
		if _, err := os.Stat(filepath.Join(candidate, "package.json")); err == nil {
			return candidate, nil
		}
	}
	return "", errors.New("the downloaded archive does not contain a Raycast extension")
}

func unzip(archive, destination string) error {
	reader, err := zip.OpenReader(archive)
	if err != nil {
		return err
	}
	defer reader.Close()
	prefix := filepath.Clean(destination) + string(os.PathSeparator)
	for _, file := range reader.File {
		target := filepath.Join(destination, file.Name)
		clean := filepath.Clean(target)
		if !strings.HasPrefix(clean, prefix) {
			return fmt.Errorf("the archive contains an unsafe path %q", file.Name)
		}
		if file.FileInfo().IsDir() {
			if err := os.MkdirAll(clean, 0o700); err != nil {
				return err
			}
			continue
		}
		if err := os.MkdirAll(filepath.Dir(clean), 0o700); err != nil {
			return err
		}
		source, err := file.Open()
		if err != nil {
			return err
		}
		destinationFile, err := os.Create(clean)
		if err != nil {
			source.Close()
			return err
		}
		_, copyErr := io.Copy(destinationFile, source)
		source.Close()
		closeErr := destinationFile.Close()
		if copyErr != nil {
			return copyErr
		}
		if closeErr != nil {
			return closeErr
		}
	}
	return nil
}
