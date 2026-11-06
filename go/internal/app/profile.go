package app

import (
	"fmt"
	"os"
	"path/filepath"
)

const previewID = "com.tinycast.win.go-preview"

type profile struct {
	settings string
	cache    string
	logs     string
	data     string
}

func previewProfile() (profile, error) {
	roaming, err := os.UserConfigDir()
	if err != nil {
		return profile{}, fmt.Errorf("preview settings directory: %w", err)
	}
	local, err := os.UserCacheDir()
	if err != nil {
		return profile{}, fmt.Errorf("preview cache directory: %w", err)
	}
	exe, err := os.Executable()
	if err != nil {
		return profile{}, err
	}
	p := profile{
		settings: filepath.Join(roaming, previewID),
		cache:    filepath.Join(local, previewID),
		logs:     filepath.Join(local, previewID, "logs"),
		data:     filepath.Join(filepath.Dir(exe), "data"),
	}
	for _, dir := range []string{p.settings, p.cache, p.logs} {
		if err := os.MkdirAll(dir, 0o700); err != nil {
			return profile{}, fmt.Errorf("create preview directory: %w", err)
		}
	}
	return p, nil
}
