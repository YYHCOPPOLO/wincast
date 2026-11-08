package app

import (
	"context"
	"errors"
	"log"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/transfer"
)

func (c *controller) copyText(text string, paste bool) {
	if c.closing || text == "" || c.pasteCancel != nil {
		return
	}
	if err := mygo.Clipboard.Write(transfer.TextData(text)); err != nil {
		c.view.Status = c.view.Text("复制失败，请重试", "Could not copy; please retry")
		log.Printf("clipboard write failed: %v", err)
		return
	}
	if err := mygo.Clipboard.Flush(); err != nil {
		c.view.Status = c.view.Text("无法保留剪贴板内容，请重试", "Could not retain clipboard contents; please retry")
		log.Printf("clipboard flush failed: %v", err)
		return
	}
	c.view.Status = c.view.Text("已复制", "Copied")
	if c.view.InDictionary() {
		c.view.PendingToast = c.view.Text("已复制词条", "Dictionary entry copied")
	}
	if !paste {
		return
	}
	target := c.target
	if err := target.Validate(); err != nil {
		c.view.Status = c.view.Text("已复制，原窗口已不可用", "Copied; the previous window is unavailable")
		log.Printf("paste target unavailable: %v", err)
		return
	}
	sequence := windows.ClipboardSequence()
	owner := c.panel.NativeHandle()
	c.hide()
	if err := target.Restore(owner); err != nil {
		c.view.Status = c.view.Text("已复制，无法恢复原窗口或权限不足", "Copied; the previous window could not be restored or requires permission")
		log.Printf("paste foreground failed: %v", err)
		c.showPanel(false)
		return
	}
	ctx, cancel := context.WithTimeout(c.ctx, 800*time.Millisecond)
	c.pasteCancel = cancel
	c.pasteID++
	id := c.pasteID
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		defer cancel()
		started := time.Now()
		err := target.Paste(ctx, sequence)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing || id != c.pasteID {
				return
			}
			c.pasteCancel = nil
			if err != nil {
				c.view.Status = c.view.Text("粘贴未发送；内容已复制，可手动粘贴", "Paste was not sent; text is copied for manual paste")
				if errors.Is(err, windows.ErrPasteUnconfirmed) {
					c.view.Status = c.view.Text("已复制；粘贴状态未确认，请检查原窗口", "Copied; paste status is unconfirmed, please check the previous window")
				}
				log.Printf("paste canceled elapsed_us=%d error=%v", time.Since(started).Microseconds(), err)
				c.showPanel(false)
				return
			}
			c.view.Status = c.view.Text("已向原窗口发送粘贴", "Paste sent to the previous window")
			log.Printf("paste sent elapsed_us=%d", time.Since(started).Microseconds())
		})
	}()
}
