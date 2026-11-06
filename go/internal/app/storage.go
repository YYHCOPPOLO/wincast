package app

import (
	"context"
	"fmt"
	"log"
	"path/filepath"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/dictionary"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
)

type lookupRequest struct {
	ctx   context.Context
	id    uint64
	query string
}

func (c *controller) startStorage() {
	c.storeStatus = c.view.Text("正在打开…", "Opening…")
	c.dictionaryStatus = c.storeStatus
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		started := time.Now()
		catalog, info, catalogErr := storage.OpenCatalog(c.ctx, filepath.Join(c.profile.cache, "catalog.sqlite3"))
		if catalog != nil {
			defer catalog.Close()
		}
		dict, err := dictionary.Open(c.ctx, c.profile.dictionary)
		if dict != nil {
			defer dict.Close()
		}
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing {
				return
			}
			if catalogErr != nil {
				c.storeStatus = c.view.Text("无法打开应用索引；原文件已保留", "Could not open the index; the original file was preserved")
				log.Printf("catalog storage open failed: %v", catalogErr)
			} else {
				c.catalogInfo = info
				c.storeStatus = "SQLite " + info.SQLiteVersion + " · WAL · FTS5/trigram"
				log.Printf("storage sqlite=%s journal=%s fts5_trigram=true elapsed_us=%d", info.SQLiteVersion, info.Journal, time.Since(started).Microseconds())
				if c.settings != nil {
					c.settings.Update(func() {})
				}
			}
			c.dictionaryReady = dict != nil
			c.view.DictionaryAvailable = c.featureDictionary && c.dictionaryReady
			if dict == nil {
				c.dictionaryStatus = c.view.Text("词典库缺失或不可读（不改写原文件）", "Dictionary missing or unreadable (original untouched)")
				log.Printf("dictionary open unavailable: %v", err)
			} else {
				c.dictionaryStatus = c.view.Text("ECDICT · 只读", "ECDICT · Read only")
				log.Printf("dictionary read_only=true open_elapsed_us=%d", time.Since(started).Microseconds())
			}
		})
		for {
			select {
			case <-c.ctx.Done():
				return
			case apps := <-c.catalogRequests:
				if catalog == nil {
					continue
				}
				ctx, cancel := context.WithTimeout(c.ctx, 2*time.Second)
				writeErr := catalog.Replace(ctx, apps)
				cancel()
				c.panel.Update(func() {
					if c.closing {
						return
					}
					if writeErr != nil {
						c.storeStatus = c.view.Text("索引保存失败；保留上次内容", "Index save failed; previous contents retained")
						log.Printf("catalog storage replace failed: %v", writeErr)
					} else {
						c.catalogInfo.Count = len(apps)
						log.Printf("catalog stored count=%d", len(apps))
						if c.settings != nil {
							c.settings.Update(func() {})
						}
					}
				})
			case request := <-c.lookupRequests:
				if dict == nil {
					continue
				}
				started := time.Now()
				entry, lookupErr := dict.Lookup(request.ctx, request.query)
				elapsed := time.Since(started)
				if c.ctx.Err() != nil {
					continue
				}
				if request.ctx.Err() != nil {
					lookupErr = request.ctx.Err()
				}
				c.panel.Update(func() {
					if c.closing || request.id != c.lookupID || !c.view.Dictionary || request.query != c.view.Query {
						return
					}
					c.lookupElapsed = elapsed
					c.lookupCount++
					c.view.DictionaryLoading = false
					if c.lookupCancel != nil {
						c.lookupCancel()
						c.lookupCancel = nil
					}
					if lookupErr != nil {
						c.view.DictionaryError = c.view.Text("查询失败，请重试", "Lookup failed; please retry")
						log.Printf("dictionary query failed: %v", lookupErr)
						return
					}
					c.view.Definition = c.definition(entry)
					log.Printf("dictionary lookup elapsed_us=%d found=%t", elapsed.Microseconds(), entry != nil)
				})
			}
		}
	}()
}

func (c *controller) persistCatalog(apps []domain.Application) {
	entries := make([]storage.Application, len(apps))
	for i, app := range apps {
		entries[i] = storage.Application{ID: app.ID, Name: app.Name, Path: app.Path}
	}
	// At most the newest complete catalog waits behind the worker.
	select {
	case <-c.catalogRequests:
	default:
	}
	c.catalogRequests <- entries
}

func (c *controller) lookup(query string) {
	if c.lookupCancel != nil {
		c.lookupCancel()
		c.lookupCancel = nil
		c.lookupCanceled++
	}
	c.lookupID++
	c.view.DictionaryLoading = false
	if !c.view.Dictionary || strings.TrimSpace(query) == "" {
		return
	}
	ctx, cancel := context.WithTimeout(c.ctx, 500*time.Millisecond)
	c.lookupCancel = cancel
	c.view.DictionaryLoading = true
	request := lookupRequest{ctx: ctx, id: c.lookupID, query: query}
	select {
	case <-c.lookupRequests:
	default:
	}
	c.lookupRequests <- request
}

func (c *controller) definition(entry *dictionary.Entry) *palette.Definition {
	if entry == nil {
		return nil
	}
	v := c.view
	return &palette.Definition{Word: entry.Word, Phonetic: entry.Phonetic, Sections: []palette.DefinitionSection{
		{Label: v.Text("中文释义", "Chinese"), Text: strings.ReplaceAll(entry.Translation, `\n`, "\n")},
		{Label: v.Text("英文释义", "English"), Text: strings.ReplaceAll(entry.Definition, `\n`, "\n")},
		{Label: v.Text("词性", "Part of speech"), Text: entry.POS},
		{Label: v.Text("词形", "Inflections"), Text: entry.Exchange},
		{Label: v.Text("标签", "Tags"), Text: entry.Tag},
		{Label: v.Text("词频与词典", "Frequency and dictionaries"), Text: fmt.Sprintf("Collins %d · Oxford %d · BNC %d · COCA %d", entry.Collins, entry.Oxford, entry.BNC, entry.Frequency)},
		{Label: v.Text("详情", "Details"), Text: entry.Detail},
		{Label: v.Text("音频", "Audio"), Text: entry.Audio},
	}}
}
