package extensions

import (
	"encoding/json"
	"fmt"
	"net/url"
	"sort"
	"strings"
)

type gitTree struct {
	Tree      []gitTreeEntry `json:"tree"`
	Truncated bool           `json:"truncated"`
}

type gitTreeEntry struct {
	Path string `json:"path"`
	Type string `json:"type"`
	SHA  string `json:"sha"`
}

// searchGitHub ranks folder names and reads only the best few manifests; a
// registry repository has thousands of folders and no search of its own.
func searchGitHub(query string) ([]Listing, error) {
	names, err := githubExtensionFolders("raycast", "extensions", "main")
	if err != nil {
		return nil, err
	}
	needle := strings.ToLower(strings.TrimSpace(query))
	ranked := make([]string, 0, len(names))
	for _, name := range names {
		if strings.Contains(strings.ToLower(name), needle) {
			ranked = append(ranked, name)
		}
	}
	sort.SliceStable(ranked, func(i, j int) bool { return ranked[i] < ranked[j] })
	if len(ranked) > 12 {
		ranked = ranked[:12]
	}
	listings := make([]Listing, 0, len(ranked))
	for _, folder := range ranked {
		listing, ok := githubListing(folder)
		if ok {
			listings = append(listings, listing)
		}
	}
	return listings, nil
}

func githubExtensionFolders(owner, repository, ref string) ([]string, error) {
	rootURL := fmt.Sprintf("https://api.github.com/repos/%s/%s/git/trees/%s", owner, repository, url.PathEscape(ref))
	data, err := httpGet(rootURL)
	if err != nil {
		return nil, err
	}
	var root gitTree
	if err := json.Unmarshal(data, &root); err != nil {
		return nil, err
	}
	sha := ""
	for _, entry := range root.Tree {
		if entry.Path == "extensions" && entry.Type == "tree" {
			sha = entry.SHA
			break
		}
	}
	if sha == "" {
		return nil, fmt.Errorf("raycast/extensions has no extensions directory")
	}
	data, err = httpGet(fmt.Sprintf("https://api.github.com/repos/%s/%s/git/trees/%s", owner, repository, sha))
	if err != nil {
		return nil, err
	}
	var tree gitTree
	if err := json.Unmarshal(data, &tree); err != nil {
		return nil, err
	}
	names := make([]string, 0, len(tree.Tree))
	for _, entry := range tree.Tree {
		if entry.Type == "tree" {
			names = append(names, entry.Path)
		}
	}
	return names, nil
}

func githubListing(folder string) (Listing, bool) {
	raw := fmt.Sprintf("https://raw.githubusercontent.com/raycast/extensions/main/extensions/%s/package.json", folder)
	data, err := httpGet(raw)
	if err != nil {
		return Listing{}, false
	}
	var manifest struct {
		Name        string `json:"name"`
		Title       string `json:"title"`
		Description string `json:"description"`
		Author      string `json:"author"`
		Icon        string `json:"icon"`
		Commands    []struct {
			Name string `json:"name"`
		} `json:"commands"`
	}
	if json.Unmarshal(data, &manifest) != nil || manifest.Name == "" {
		return Listing{}, false
	}
	icon := ""
	if manifest.Icon != "" {
		icon = fmt.Sprintf("https://raw.githubusercontent.com/raycast/extensions/main/extensions/%s/assets/%s", folder, manifest.Icon)
	}
	title := manifest.Title
	if title == "" {
		title = manifest.Name
	}
	return Listing{
		ID: "raycast-extensions/" + folder, Name: manifest.Name, Title: title,
		Summary: manifest.Description, Author: manifest.Author,
		LightIconURL: icon, DarkIconURL: icon, CommandCount: len(manifest.Commands),
		Registry: "raycast/extensions",
		Source: ListingSource{
			Kind: "github", Owner: "raycast", Repository: "extensions",
			Path: "extensions/" + folder, Ref: "main",
		},
	}, true
}
