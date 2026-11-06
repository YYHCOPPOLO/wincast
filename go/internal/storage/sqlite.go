package storage

import (
	"context"
	"database/sql"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	_ "modernc.org/sqlite"
)

// OpenSQLite enforces the access mode on every connection. Opening a missing
// read-only dictionary cannot silently create an empty replacement database.
func OpenSQLite(ctx context.Context, path string, readOnly bool) (*sql.DB, error) {
	abs, err := filepath.Abs(path)
	if err != nil {
		return nil, err
	}
	if readOnly {
		info, err := os.Stat(abs)
		if err != nil {
			return nil, err
		}
		if !info.Mode().IsRegular() {
			return nil, fmt.Errorf("database is not a regular file")
		}
	} else if err := os.MkdirAll(filepath.Dir(abs), 0o700); err != nil {
		return nil, err
	}
	uriPath := filepath.ToSlash(abs)
	if !strings.HasPrefix(uriPath, "/") {
		uriPath = "/" + uriPath
	}
	uri := url.URL{Scheme: "file", Path: uriPath}
	params := url.Values{"_pragma": {"busy_timeout(250)", "foreign_keys(ON)"}}
	if readOnly {
		params.Set("mode", "ro")
		params.Add("_pragma", "query_only(ON)")
	} else {
		params.Set("mode", "rwc")
	}
	uri.RawQuery = params.Encode()
	database, err := sql.Open("sqlite", uri.String())
	if err != nil {
		return nil, err
	}
	database.SetMaxOpenConns(1)
	database.SetMaxIdleConns(1)
	if err := database.PingContext(ctx); err != nil {
		database.Close()
		return nil, err
	}
	return database, nil
}
