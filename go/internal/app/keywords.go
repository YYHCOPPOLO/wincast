package app

import (
	"errors"
	"log"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/snippet"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/template"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/snippets"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

// keywordList is the enabled keyword set, ordered by file identity so
// duplicate keywords resolve deterministically.
func (c *controller) keywordList() []string {
	keywords := make([]string, 0, len(c.snippetSnapshot.Records))
	for _, record := range c.snippetSnapshot.Records {
		if !record.Enabled {
			continue
		}
		keyword := strings.TrimSpace(record.Keyword)
		if keyword != "" {
			keywords = append(keywords, keyword)
		}
	}
	return keywords
}

// applyKeywordListener installs the single keyword listener while snippets
// are enabled. Disabling clears the buffer and removes the hook.
func (c *controller) applyKeywordListener() {
	if c.expander == nil {
		c.expander = snippets.NewExpander()
	}
	c.snippetsEnabled.Store(c.featureSnippets)
	if c.featureSnippets && c.messageHost != nil {
		c.expander.SetKeywords(c.keywordList())
		c.expander.SetEnabled(true)
		if c.keywordStop == nil {
			stop, err := c.messageHost.AddKeyboardListener(c.onKeywordKey)
			if err != nil {
				log.Printf("snippet keyword listener unavailable: %v", err)
				c.expander.SetEnabled(false)
				return
			}
			c.keywordStop = stop
		}
		log.Printf("snippet keyword listener active keywords=%d", len(c.keywordList()))
		return
	}
	c.expander.SetEnabled(false)
	c.expander.Reset()
	if c.keywordStop != nil {
		c.keywordStop()
		c.keywordStop = nil
		log.Printf("snippet keyword listener stopped")
	}
}

// onKeywordKey runs on the message-host thread. It reads only the character
// and flags the buffer needs; nothing is logged or persisted from input.
func (c *controller) onKeywordKey(event windows.KeyEvent) {
	if c.expander == nil || !c.expander.Enabled() {
		return
	}
	synthetic := event.Injected || windows.SyntheticTag(event.ExtraInfo)
	var character rune
	producesText := false
	if !synthetic && event.Down {
		character, producesText = windows.KeyText(event)
	}
	input := snippets.Input{
		Synthetic:      synthetic,
		KeyDown:        event.Down,
		Commanding:     event.Down && windows.CommandingDown(),
		ResetKey:       windows.ResetVirtualKey(event.VirtualKey),
		DeleteBackward: event.VirtualKey == windows.BackspaceVirtualKey,
		ProducesText:   producesText,
		Composing:      !synthetic && windows.IMEComposing(),
		Character:      character,
		Foreground:     windows.ForegroundWindow(),
		NowMS:          time.Now().UnixMilli(),
	}
	keyword, matched := c.expander.Feed(input)
	if !matched {
		return
	}
	if c.panel == nil {
		return
	}
	c.panel.Update(func() {
		if c.closing {
			return
		}
		c.handleKeyword(keyword)
	})
}

// handleKeyword expands a matched keyword in the window it was typed in. A
// failed consent, target or argument gate leaves the typed keyword untouched.
func (c *controller) handleKeyword(keyword string) {
	if c.closing || !c.snippetsEnabled.Load() {
		return
	}
	record, found := c.snippetForKeyword(keyword)
	if !found {
		return
	}
	target := windows.ForegroundTarget()
	if target.Window == 0 {
		return // Wincast itself or a system surface: never inject there.
	}
	captured := c.captureTemplateContext()
	output := template.ExpandSnippet(record.Body, captured, nil, c.snippetList(), record.ID)
	if len(output.Arguments) > 0 {
		// A keyword expansion cannot prompt without stealing focus from the
		// target; the typed keyword stays exactly as written.
		log.Printf("snippet keyword skipped reason=arguments name=%q", record.Name)
		return
	}
	if strings.TrimSpace(output.Text) == "" {
		return
	}
	c.keywordDelivery++
	delivery := c.keywordDelivery
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		if !c.snippetsEnabled.Load() {
			return
		}
		if err := deliverKeyword(target, keyword, output, func() bool { return c.snippetsEnabled.Load() }); err != nil {
			if c.ctx.Err() == nil {
				log.Printf("snippet keyword delivery failed: %v", err)
			}
			return
		}
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing || delivery != c.keywordDelivery || !c.snippetsEnabled.Load() {
				return
			}
			log.Printf("snippet keyword delivered name=%q", record.Name)
			if record.ShowConfirmation {
				c.showToast(c.view.Text("已展开片段："+record.Name, "Snippet expanded: "+record.Name))
			}
		})
	}()
}

func (c *controller) snippetForKeyword(keyword string) (snippet.Snippet, bool) {
	for _, record := range c.snippetSnapshot.Records {
		if record.Enabled && strings.EqualFold(strings.TrimSpace(record.Keyword), strings.TrimSpace(keyword)) {
			return record, true
		}
	}
	return snippet.Snippet{}, false
}

// deliverKeyword deletes the typed keyword and types the expansion. Every
// gate is rechecked immediately before injection.
func deliverKeyword(target windows.Target, keyword string, output template.Output, consent func() bool) error {
	if err := target.CheckEditable(); err != nil {
		return err
	}
	if foreground := windows.ForegroundWindow(); foreground != target.Window {
		return errForegroundChanged
	}
	if !consent() {
		return errors.New("snippet expansion was disabled")
	}
	count := len([]rune(strings.TrimSpace(keyword)))
	if count > 0 {
		if err := windows.SendBackspaces(count); err != nil {
			return err
		}
		time.Sleep(25 * time.Millisecond)
		if err := target.CheckEditable(); err != nil {
			return err
		}
		if foreground := windows.ForegroundWindow(); foreground != target.Window {
			return errForegroundChanged
		}
		if !consent() {
			return errors.New("snippet expansion was disabled")
		}
	}
	if err := windows.SendUnicodeText(output.Text); err != nil {
		return err
	}
	if output.HasCursor {
		if total := template.GraphemeCount(output.Text); output.Cursor < total {
			time.Sleep(20 * time.Millisecond)
			_ = windows.SendLeftArrows(total - output.Cursor)
		}
	}
	return nil
}

type deliveryError string

func (e deliveryError) Error() string { return string(e) }

const errForegroundChanged deliveryError = "the foreground window changed before delivery"
