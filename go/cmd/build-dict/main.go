// Command build-dict streams ECDICT CSV into Wincast's local dictionary.
package main

import (
	"context"
	"database/sql"
	"encoding/csv"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	_ "modernc.org/sqlite"
)

func main() {
	input := flag.String("input", "", "ECDICT CSV path")
	output := flag.String("output", "", "destination SQLite path")
	flag.Parse()
	if *input == "" || *output == "" {
		flag.Usage()
		os.Exit(2)
	}
	if err := build(*input, *output); err != nil {
		log.Fatal(err)
	}
}

func build(input, output string) error {
	started := time.Now()
	file, err := os.Open(input)
	if err != nil {
		return err
	}
	defer file.Close()
	reader := csv.NewReader(file)
	reader.FieldsPerRecord = -1
	header, err := reader.Read()
	if err != nil {
		return err
	}
	columns := []string{"word", "phonetic", "definition", "translation", "pos", "collins", "oxford", "tag", "bnc", "frq", "exchange", "detail", "audio"}
	if len(header) < len(columns) {
		return errors.New("ECDICT header has too few columns")
	}
	for i, column := range columns {
		if strings.TrimPrefix(header[i], "\ufeff") != column {
			return fmt.Errorf("unexpected ECDICT column %d: %q", i+1, header[i])
		}
	}
	if err := os.MkdirAll(filepath.Dir(output), 0o700); err != nil {
		return err
	}
	part, err := os.CreateTemp(filepath.Dir(output), ".wincast-dictionary-*.sqlite")
	if err != nil {
		return err
	}
	temporary := part.Name()
	if err := part.Close(); err != nil {
		os.Remove(temporary)
		return err
	}
	defer os.Remove(temporary)
	db, err := sql.Open("sqlite", temporary)
	if err != nil {
		return err
	}
	defer db.Close()
	db.SetMaxOpenConns(1)
	if _, err := db.Exec(`PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA page_size=8192;
CREATE TABLE words (word TEXT PRIMARY KEY, sw TEXT NOT NULL,
phonetic TEXT, definition TEXT, translation TEXT, pos TEXT, collins INTEGER,
oxford INTEGER, tag TEXT, bnc INTEGER, frq INTEGER, exchange TEXT, detail TEXT, audio TEXT);`); err != nil {
		return err
	}
	tx, err := db.BeginTx(context.Background(), nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	stmt, err := tx.Prepare("INSERT OR REPLACE INTO words VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
	if err != nil {
		return err
	}
	defer stmt.Close()
	count := 0
	for {
		record, err := reader.Read()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return err
		}
		value := func(i int) string {
			if i < len(record) {
				return strings.TrimSpace(record[i])
			}
			return ""
		}
		word := value(0)
		if word == "" {
			continue
		}
		var key strings.Builder
		for _, r := range word {
			if r >= 'A' && r <= 'Z' {
				r += 'a' - 'A'
			}
			if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') {
				key.WriteRune(r)
			}
		}
		values := []any{word, key.String()}
		for i := 1; i < len(columns); i++ {
			if i == 5 || i == 6 || i == 8 || i == 9 {
				n, _ := strconv.ParseInt(value(i), 10, 64)
				values = append(values, n)
			} else {
				values = append(values, value(i))
			}
		}
		if _, err := stmt.Exec(values...); err != nil {
			return err
		}
		count++
	}
	if count == 0 {
		return errors.New("ECDICT CSV contains no entries")
	}
	if err := tx.Commit(); err != nil {
		return err
	}
	if _, err := db.Exec("CREATE INDEX words_sw ON words(sw); CREATE INDEX words_nocase ON words(word COLLATE NOCASE); VACUUM;"); err != nil {
		return err
	}
	if err := db.Close(); err != nil {
		return err
	}
	if err := os.Rename(temporary, output); err != nil {
		return err
	}
	fmt.Printf("Wrote %d entries to %s in %s\n", count, output, time.Since(started).Round(time.Millisecond))
	return nil
}
