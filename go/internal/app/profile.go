package app

import (
	"fmt"

	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

const previewID = storage.PreviewID
const appName = "Tinycast"

type profile struct {
	settings   string
	cache      string
	logs       string
	data       string
	dictionary string
}

// previewProfile resolves the product identity. It never falls back
// to the legacy application's writable directories; only the external
// dictionary is shared read-only.
func previewProfile() (profile, error) {
	roots, err := storage.ResolveRoots(storage.Preview)
	if err != nil {
		return profile{}, fmt.Errorf("resolve preview paths: %w", err)
	}
	if err := roots.Ensure(); err != nil {
		return profile{}, err
	}
	return profile{
		settings: roots.Settings, cache: roots.Cache, logs: roots.Logs,
		data: roots.Data, dictionary: roots.Dictionary,
	}, nil
}
