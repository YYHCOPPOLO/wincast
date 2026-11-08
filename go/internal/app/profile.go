package app

import (
	"fmt"

	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

const appID = storage.AppID
const appName = "Wincast"

type profile struct {
	settings   string
	cache      string
	logs       string
	data       string
	dictionary string
}

// appProfile resolves the product identity. The previous Go profile is
// copied once; a previously installed dictionary stays available read-only.
func appProfile() (profile, error) {
	roots, err := storage.ResolveRoots()
	if err != nil {
		return profile{}, fmt.Errorf("resolve application paths: %w", err)
	}
	if err := roots.Ensure(); err != nil {
		return profile{}, err
	}
	return profile{
		settings: roots.Settings, cache: roots.Cache, logs: roots.Logs,
		data: roots.Data, dictionary: roots.Dictionary,
	}, nil
}
