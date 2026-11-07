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
				var suggestions []string
				if lookupErr == nil && entry == nil {
					suggestions, _ = dict.Suggestions(request.ctx, request.query, 8)
				}
				elapsed := time.Since(started)
				if c.ctx.Err() != nil {
					continue
				}
				if request.ctx.Err() != nil {
					lookupErr = request.ctx.Err()
				}
				c.panel.Update(func() {
					if c.closing || request.id != c.lookupID || c.view.Mode != palette.ModeDictionary || request.query != c.view.Query {
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
					c.view.Suggestions = suggestions
					log.Printf("dictionary lookup elapsed_us=%d found=%t suggestions=%d", elapsed.Microseconds(), entry != nil, len(suggestions))
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
	if c.view.Mode != palette.ModeDictionary || strings.TrimSpace(query) == "" {
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
	var badges []string
	if entry.Collins > 0 {
		stars := int(entry.Collins)
		if stars > 5 {
			stars = 5
		}
		badges = append(badges, fmt.Sprintf("%s %s", v.Text("柯林斯", "Collins"), strings.Repeat("★", stars)))
	}
	if entry.Oxford > 0 {
		badges = append(badges, v.Text("牛津", "Oxford"))
	}
	for _, tag := range strings.Fields(entry.Tag) {
		badges = append(badges, tag)
	}
	return &palette.Definition{Word: entry.Word, Phonetic: entry.Phonetic, Badges: badges, Sections: []palette.DefinitionSection{
		{Label: v.Text("中文释义", "Chinese"), Text: dictionaryLines(entry.Translation)},
		{Label: v.Text("英文释义", "English"), Text: dictionaryLines(entry.Definition)},
		{Label: v.Text("词性", "Part of speech"), Text: formatDictionaryPOS(entry.POS)},
		{Label: v.Text("词形", "Inflections"), Text: formatDictionaryExchange(entry.Exchange, v.Chinese)},
		{Label: v.Text("标签", "Tags"), Text: entry.Tag},
		{Label: v.Text("词频与词典", "Frequency and dictionaries"), Text: fmt.Sprintf("Collins %d · Oxford %d · BNC %d · COCA %d", entry.Collins, entry.Oxford, entry.BNC, entry.Frequency)},
		{Label: v.Text("详情", "Details"), Text: dictionaryLines(entry.Detail)},
		{Label: v.Text("音频", "Audio"), Text: dictionaryLines(entry.Audio)},
	}}
}

func dictionaryLines(raw string) string {
	raw = strings.ReplaceAll(raw, `\r`, "\n")
	raw = strings.ReplaceAll(raw, `\n`, "\n")
	var lines []string
	for _, line := range strings.Split(raw, "\n") {
		line = strings.TrimSpace(line)
		if line != "" && line != `""` {
			lines = append(lines, line)
		}
	}
	return strings.Join(lines, "\n")
}

func formatDictionaryPOS(raw string) string {
	var parts []string
	for _, token := range strings.Split(strings.ReplaceAll(raw, `\`, " "), "/") {
		token = strings.TrimSpace(token)
		if token == "" {
			continue
		}
		if kind, share, ok := strings.Cut(token, ":"); ok && strings.TrimSpace(share) != "" {
			parts = append(parts, strings.TrimSpace(kind)+" "+strings.TrimSpace(share)+"%")
		} else {
			parts = append(parts, token)
		}
	}
	return strings.Join(parts, " · ")
}

func formatDictionaryExchange(raw string, chinese bool) string {
	labels := map[byte]string{
		'0': "Lemma", 'p': "Past", 'd': "Past participle", 'i': "Present participle",
		'3': "Third person singular", 'r': "Comparative", 't': "Superlative",
		's': "Plural", 'f': "Alternate plural",
	}
	if chinese {
		labels = map[byte]string{
			'0': "原形", 'p': "过去式", 'd': "过去分词", 'i': "现在分词",
			'3': "第三人称单数", 'r': "比较级", 't': "最高级", 's': "复数", 'f': "复数变体",
		}
	}
	var lines []string
	for _, item := range strings.Split(raw, "/") {
		code, value, ok := strings.Cut(item, ":")
		code, value = strings.TrimSpace(code), strings.TrimSpace(value)
		if !ok || value == "" {
			continue
		}
		if code == "1" {
			var derived []string
			for _, c := range value {
				if label, ok := labels[byte(c)]; ok {
					derived = append(derived, label)
				}
			}
			if len(derived) > 0 {
				lines = append(lines, strings.Join(derived, " / "))
			}
			continue
		}
		if label, ok := labels[code[0]]; ok && len(code) == 1 {
			lines = append(lines, label+" "+value)
		}
	}
	return strings.Join(lines, "\n")
}
