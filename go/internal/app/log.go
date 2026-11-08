package app

import (
	"errors"
	"os"
	"path/filepath"
)

// Logs are bounded across normal starts. Rotation owns only these two fixed
// files inside this preview's resolved log directory, never a directory tree.
func openLog(directory string) (*os.File, error) {
	path := filepath.Join(directory, "tinycast.log")
	info, err := os.Stat(path)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return nil, err
	}
	if err == nil && info.Size() >= 2*1024*1024 {
		previous := filepath.Join(directory, "tinycast.previous.log")
		if err := os.Remove(previous); err != nil && !errors.Is(err, os.ErrNotExist) {
			return nil, err
		}
		if err := os.Rename(path, previous); err != nil {
			return nil, err
		}
	}
	return os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
}
