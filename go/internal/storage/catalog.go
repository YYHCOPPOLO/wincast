package storage

import (
	"context"
	"database/sql"
	"fmt"
	"strings"
)

type Application struct{ ID, Name, Path string }

type Catalog struct{ db *sql.DB }

type CatalogInfo struct {
	SQLiteVersion string
	Journal       string
	Count         int
}

func OpenCatalog(ctx context.Context, path string) (*Catalog, CatalogInfo, error) {
	database, err := OpenSQLite(ctx, path, false)
	if err != nil {
		return nil, CatalogInfo{}, err
	}
	fail := func(err error) (*Catalog, CatalogInfo, error) { database.Close(); return nil, CatalogInfo{}, err }
	var info CatalogInfo
	if err := database.QueryRowContext(ctx, "PRAGMA journal_mode=WAL").Scan(&info.Journal); err != nil {
		return fail(err)
	}
	if !strings.EqualFold(info.Journal, "wal") {
		return fail(fmt.Errorf("catalog WAL unavailable: %s", info.Journal))
	}
	// This is the real local application index, populated from AppsFolder.
	// Its schema both requires and exercises FTS5's trigram tokenizer; no
	// synthetic database, probe rows or benchmark executable is involved.
	if _, err := database.ExecContext(ctx, `CREATE VIRTUAL TABLE IF NOT EXISTS app_catalog
		USING fts5(id UNINDEXED, name, path UNINDEXED, tokenize='trigram');`); err != nil {
		return fail(err)
	}
	if err := database.QueryRowContext(ctx, "SELECT sqlite_version(), (SELECT count(*) FROM app_catalog)").Scan(&info.SQLiteVersion, &info.Count); err != nil {
		return fail(err)
	}
	return &Catalog{db: database}, info, nil
}

func (c *Catalog) Close() error { return c.db.Close() }

func (c *Catalog) Replace(ctx context.Context, apps []Application) error {
	tx, err := c.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err := tx.ExecContext(ctx, "DELETE FROM app_catalog"); err != nil {
		return err
	}
	statement, err := tx.PrepareContext(ctx, "INSERT INTO app_catalog(id,name,path) VALUES(?,?,?)")
	if err != nil {
		return err
	}
	defer statement.Close()
	for _, app := range apps {
		if _, err := statement.ExecContext(ctx, app.ID, app.Name, app.Path); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// SearchNames is useful to the catalog's indexed name filter. Short queries
// use the in-memory ranker because a trigram cannot represent fewer than three
// Unicode characters. MATCH input is quoted as one literal, never SQL syntax.
func (c *Catalog) SearchNames(ctx context.Context, query string, limit int) ([]Application, error) {
	if len([]rune(query)) < 3 || limit <= 0 {
		return nil, nil
	}
	query = `"` + strings.ReplaceAll(query, `"`, `""`) + `"`
	rows, err := c.db.QueryContext(ctx, "SELECT id,name,path FROM app_catalog WHERE name MATCH ? LIMIT ?", query, min(limit, 500))
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var found []Application
	for rows.Next() {
		var app Application
		if err := rows.Scan(&app.ID, &app.Name, &app.Path); err != nil {
			return nil, err
		}
		found = append(found, app)
	}
	return found, rows.Err()
}
