package storage

import (
	"fmt"
	"os"
	"path/filepath"
)

// RootKind separates the development preview from the daily application. The
// two never share a writable settings, cache, data or log directory.
type RootKind uint8

const (
	Preview RootKind = iota
	Daily
)

const (
	PreviewID = "com.tinycast.win.go-preview"
	DailyID   = "com.tinycast.win"
)

// Roots are the resolved directories for one application identity.
type Roots struct {
	Settings   string
	Cache      string
	Logs       string
	Data       string
	Dictionary string
}

func ResolveRoots(kind RootKind) (Roots, error) {
	roaming, err := os.UserConfigDir()
	if err != nil {
		return Roots{}, fmt.Errorf("resolve roaming directory: %w", err)
	}
	local, err := os.UserCacheDir()
	if err != nil {
		return Roots{}, fmt.Errorf("resolve local directory: %w", err)
	}
	executable, err := os.Executable()
	if err != nil {
		return Roots{}, err
	}
	id := PreviewID
	if kind == Daily {
		id = DailyID
	}
	return Roots{
		Settings:   filepath.Join(roaming, id),
		Cache:      filepath.Join(local, id),
		Logs:       filepath.Join(local, id, "logs"),
		Data:       filepath.Join(filepath.Dir(executable), "data"),
		Dictionary: filepath.Join(roaming, DailyID, "Dictionary", "ecdict.sqlite"),
	}, nil
}

// Ensure creates only the writable roots. The dictionary stays read-only and
// is never created here.
func (r Roots) Ensure() error {
	for _, directory := range []string{r.Settings, r.Cache, r.Logs} {
		if err := os.MkdirAll(directory, 0o700); err != nil {
			return fmt.Errorf("create application directory: %w", err)
		}
	}
	return nil
}
