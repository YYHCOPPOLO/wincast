package storage

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

const (
	LegacyGoID = "com.tinycast.win.go-preview"
	LegacyID   = "com.tinycast.win"
	AppID      = "com.wincast.win"
)

// Roots are the resolved directories for one application identity.
type Roots struct {
	Settings   string
	Cache      string
	Logs       string
	Data       string
	Dictionary string
}

func ResolveRoots() (Roots, error) {
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
	settings := filepath.Join(roaming, AppID)
	if err := importPreviousProfile(settings, filepath.Join(roaming, LegacyGoID)); err != nil {
		return Roots{}, fmt.Errorf("import previous Go settings: %w", err)
	}
	dictionary := filepath.Join(local, AppID, "Dictionary", "ecdict.sqlite")
	if _, err := os.Stat(dictionary); errors.Is(err, os.ErrNotExist) {
		dictionary = filepath.Join(roaming, LegacyID, "Dictionary", "ecdict.sqlite")
	}
	return Roots{
		Settings:   settings,
		Cache:      filepath.Join(local, AppID),
		Logs:       filepath.Join(local, AppID, "logs"),
		Data:       filepath.Join(filepath.Dir(executable), "data"),
		Dictionary: dictionary,
	}, nil
}

// importPreviousProfile stages a copy on first launch. Never overwrite an
// existing Wincast profile or remove the previous profile, including AI keys.
func importPreviousProfile(destination, previous string) error {
	if _, err := os.Stat(destination); err == nil {
		return nil
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if _, err := os.Stat(previous); errors.Is(err, os.ErrNotExist) {
		return nil
	} else if err != nil {
		return err
	}
	staging, err := os.MkdirTemp(filepath.Dir(destination), ".wincast-profile-*")
	if err != nil {
		return err
	}
	defer os.RemoveAll(staging)
	if err := os.CopyFS(staging, os.DirFS(previous)); err != nil {
		return err
	}
	if err := os.Rename(staging, destination); err != nil {
		// Another first launch may have completed the same import.
		if info, statErr := os.Stat(destination); statErr == nil && info.IsDir() {
			return nil
		}
		return err
	}
	return nil
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
