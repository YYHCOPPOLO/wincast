package app

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"image"
	"image/png"
	"io"
	"log"
	"os"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/features/clipboard"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
	imagedraw "golang.org/x/image/draw"
)

// startClipboard opens the history store on a worker; no SQLite call runs on
// the UI loop. The message host only signals this worker after a clipboard
// notification, keeping the Win32 message thread responsive.
func (c *controller) startClipboard() {
	disabled := append([]string(nil), c.clipboardDisabled...)
	retention := c.clipboardRetention
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		manager, err := clipboard.NewManager(c.ctx, c.profile.data, "", disabled)
		if err != nil {
			c.panel.Update(func() {
				if c.closing {
					return
				}
				c.clipboardError = c.view.Text("剪贴板存储不可用，已有历史记录已保留", "Clipboard storage is unavailable; existing history is preserved")
				c.view.ClipboardError = c.clipboardError
				c.view.ClipboardAvailable = false
				log.Printf("clipboard storage unavailable: %v", err)
				if c.settings != nil {
					c.settings.Update(func() {})
				}
			})
			return
		}
		defer manager.Close()
		manager.ApplyRetention(retention)
		c.panel.Update(func() {
			if c.closing {
				return
			}
			c.clipboard = manager
			c.clipboardReady = true
			c.clipboardError = manager.Store().LastError()
			items := manager.Store().Search("", clipboard.FilterAll)
			c.view.ClipboardAvailable = manager.Store().Available()
			c.view.ClipboardError = c.clipboardError
			if c.view.Clipboard() {
				c.view.RefreshClipboard()
			}
			log.Printf("clipboard storage ready dir=%s retention_days=%d items=%d", c.profile.data, c.clipboardRetention, len(items))
			if c.settings != nil {
				c.settings.Update(func() {})
			}
		})
		for {
			select {
			case <-c.ctx.Done():
				return
			case <-c.clipboardRequests:
				if c.ctx.Err() != nil || !c.clipboardEnabled.Load() {
					continue
				}
				before := c.clipboardEvents.Load()
				previousError := manager.Store().LastError()
				changed := manager.Capture()
				if !changed && manager.Store().LastError() == previousError {
					continue
				}
				if changed {
					log.Printf("clipboard captured")
					c.clipboardEvents.Store(before + 1)
				} else {
					log.Printf("clipboard capture failed: %s", manager.Store().LastError())
				}
				c.panel.Update(func() {
					if c.closing {
						return
					}
					c.clipboardError = manager.Store().LastError()
					c.view.ClipboardError = c.clipboardError
					c.view.ClipboardAvailable = manager.Store().Available()
					if c.view.Clipboard() {
						c.view.RefreshClipboard()
					}
					if c.settings != nil {
						c.settings.Update(func() {})
					}
				})
			}
		}
	}()
}

// startClipboardHost owns the Win32 message window for clipboard and theme
// broadcasts. The callback only signals the storage worker.
func (c *controller) startClipboardHost() {
	host, err := windows.StartMessageHost()
	if err != nil {
		log.Printf("message host unavailable: %v", err)
		return
	}
	c.messageHost = host
	if err := host.WatchClipboard(func() {
		if !c.clipboardEnabled.Load() {
			return
		}
		select {
		case c.clipboardRequests <- struct{}{}:
		default:
		}
	}); err != nil {
		log.Printf("clipboard listener unavailable: %v", err)
	}
	if err := host.WatchTheme(func(dark bool) {
		c.panel.Update(func() {
			if c.closing {
				return
			}
			c.systemDark = dark
			if c.settings != nil {
				c.settings.Update(func() {})
			}
		})
	}); err != nil {
		log.Printf("system theme listener unavailable: %v", err)
	}
}

func (c *controller) clipboardSummary() string {
	if c.clipboard != nil {
		items := c.clipboard.Store().Search("", clipboard.FilterAll)
		return fmt.Sprintf("%d", len(items))
	}
	events := c.clipboardEvents.Load()
	if c.language == "zh" {
		return fmt.Sprintf("%d 次", events)
	}
	return fmt.Sprintf("%d events", events)
}

func (c *controller) clearClipboardHistory() {
	if c.clipboard == nil || c.clipboardConfirm || c.closing {
		return
	}
	c.clipboardConfirm = true
	parent := c.panel
	if c.settings != nil && windows.ForegroundWindow() == c.settings.NativeHandle() {
		parent = c.settings
	}
	options := mygo.MessageOptions{
		Parent: parent, Type: mygo.MessageWarning,
		Title:         c.text("清空剪贴板历史？", "Clear clipboard history?"),
		Message:       c.text("删除全部历史记录，包括置顶内容和图片？", "Delete every entry, including pinned items and images?"),
		Detail:        c.text("此操作无法撤销。", "This action cannot be undone."),
		Buttons:       []string{c.text("取消", "Cancel"), c.text("清空全部历史", "Clear all history")},
		DefaultButton: 0, CancelButton: 0,
	}
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		result, err := mygo.Dialog.Message(options)
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			c.clipboardConfirm = false
			if c.closing {
				return
			}
			if err != nil {
				log.Printf("clipboard clear confirmation failed: %v", err)
				return
			}
			if result.Button == 1 {
				c.clearClipboardHistoryConfirmed()
			}
		})
	}()
}

func (c *controller) clearClipboardHistoryConfirmed() {
	if !c.clipboard.Clear() {
		c.clipboardError = c.view.Text("无法清空剪贴板历史", "Could not clear clipboard history")
		log.Printf("clipboard clear failed: %v", c.clipboard.Store().LastError())
		return
	}
	c.clipboardError = ""
	c.view.ClipboardError = ""
	c.view.RefreshClipboard()
	if c.settings != nil {
		c.settings.Update(func() {})
	}
}

func (c *controller) setClipboardRetention(days int64) {
	c.clipboardRetention = days
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("clipboardRetentionDays", days)
	}
	c.queueSettingsSave()
	if c.clipboard != nil {
		c.clipboard.ApplyRetention(days)
		c.view.RefreshClipboard()
	}
}

// Clipboard mode is added by the palette in this plan.
func (c *controller) openClipboard() {
	if c.closing {
		return
	}
	c.view.SetModeClipboard()
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
	}
	c.showPanel(false)
}

type clipboardImageRequest struct {
	id   string
	path string
}

func (c *controller) startClipboardImages() {
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		for {
			select {
			case <-c.ctx.Done():
				return
			case request := <-c.clipboardImageRequests:
				if c.ctx.Err() != nil {
					return
				}
				bitmap, err := loadClipboardPreview(request.path)
				if err != nil {
					log.Printf("clipboard preview unavailable: %v", err)
				}
				if c.ctx.Err() != nil {
					return
				}
				c.panel.Update(func() {
					if c.closing {
						return
					}
					delete(c.clipboardPending, request.id)
					bytes := clipboardBitmapBytes(bitmap)
					for len(c.clipboardImages) >= 32 || c.clipboardImageBytes+bytes > 48*1024*1024 {
						for old := range c.clipboardImages {
							c.clipboardImageBytes -= clipboardBitmapBytes(c.clipboardImages[old])
							delete(c.clipboardImages, old)
							break
						}
					}
					c.clipboardImages[request.id] = bitmap
					c.clipboardImageBytes += bytes
				})
			}
		}
	}()
}

func (c *controller) clipboardBitmap(item clipboard.Item) (*ui.Bitmap, bool) {
	if item.Kind != clipboard.KindImage || item.ImagePath == "" {
		return nil, false
	}
	if bitmap, ok := c.clipboardImages[item.ID]; ok {
		return bitmap, false
	}
	if !c.clipboardPending[item.ID] {
		select {
		case c.clipboardImageRequests <- clipboardImageRequest{id: item.ID, path: item.ImagePath}:
			c.clipboardPending[item.ID] = true
		default:
		}
	}
	return nil, true
}

// Preview thumbnails are independent of the original PNG used for copying.
// Bound compressed bytes and decoded pixels before allocating an image.
func loadClipboardPreview(path string) (*ui.Bitmap, error) {
	const maxBytes = 64 * 1024 * 1024
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil || !info.Mode().IsRegular() || info.Size() <= 0 || info.Size() > maxBytes {
		return nil, errors.New("image preview file is unavailable or too large")
	}
	data, err := io.ReadAll(io.LimitReader(file, maxBytes+1))
	if err != nil {
		return nil, err
	}
	if len(data) > maxBytes {
		return nil, errors.New("image preview exceeds the file limit")
	}
	config, err := png.DecodeConfig(bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	if config.Width <= 0 || config.Height <= 0 || config.Width > 8192 || config.Height > 8192 ||
		int64(config.Width)*int64(config.Height) > maxBytes/4 {
		return nil, errors.New("image preview exceeds the pixel limit")
	}
	source, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	width, height := config.Width, config.Height
	if max(width, height) <= 900 {
		return ui.NewBitmap(source), nil
	}
	if width >= height {
		height, width = max(1, height*900/width), 900
	} else {
		width, height = max(1, width*900/height), 900
	}
	thumbnail := image.NewRGBA(image.Rect(0, 0, width, height))
	imagedraw.CatmullRom.Scale(thumbnail, thumbnail.Bounds(), source, source.Bounds(), imagedraw.Src, nil)
	return ui.NewBitmap(thumbnail), nil
}

func clipboardBitmapBytes(bitmap *ui.Bitmap) int64 {
	if bitmap == nil {
		return 0
	}
	width, height := bitmap.Size()
	// Include the CPU copy, GPU texture and smaller rendering levels.
	return int64(width) * int64(height) * 4 * 3
}

func (c *controller) clipboardSearch(query string, filter clipboard.Filter) []clipboard.Item {
	c.clipboardFilter = filter
	if c.clipboard == nil {
		return nil
	}
	return c.clipboard.Store().Search(query, filter)
}

func (c *controller) setClipboardFilter(filter clipboard.Filter) {
	c.clipboardFilter = filter
}

func (c *controller) refreshClipboardView() {
	if c.view.Mode == palette.ModeClipboard {
		c.view.RefreshClipboard()
	}
	if c.settings != nil {
		c.settings.Update(func() {})
	}
}

func (c *controller) copyClipboardItem(item clipboard.Item) {
	if c.writeClipboardItem(item) {
		c.view.Status = c.view.Text("已复制", "Copied")
		c.refreshClipboardView()
	}
}

func (c *controller) writeClipboardItem(item clipboard.Item) bool {
	if c.closing {
		return false
	}
	var err error
	switch item.Kind {
	case clipboard.KindImage:
		data, readErr := os.ReadFile(item.ImagePath)
		if readErr != nil {
			err = readErr
			break
		}
		err = windows.WriteClipboardPNG(c.panel.NativeHandle(), data, true)
	default:
		err = windows.WriteClipboardText(c.panel.NativeHandle(), item.Text, true)
	}
	if err != nil {
		c.view.Status = c.view.Text("复制失败，请重试", "Could not copy; please retry")
		log.Printf("clipboard item write failed: %v", err)
		return false
	}
	if c.clipboard != nil && !item.Pinned() {
		c.clipboard.Store().Promote(item.ID)
	}
	return true
}

func (c *controller) pasteClipboardItem(item clipboard.Item) {
	if c.pasteCancel != nil {
		return
	}
	if !c.writeClipboardItem(item) {
		return
	}
	c.pasteToRememberedTarget()
}

func (c *controller) pasteClipboardSlot(digit byte) {
	if c.clipboard == nil {
		return
	}
	index := int(digit - '1')
	if digit == '0' {
		index = 9
	}
	item, ok := c.clipboard.Store().PinnedItem(index, c.view.Query, c.clipboardFilter)
	if !ok {
		c.view.Status = c.view.Text("该收藏槽为空", "That pinned slot is empty")
		return
	}
	c.pasteClipboardItem(item)
}

func (c *controller) toggleClipboardPin(item clipboard.Item) {
	if c.clipboard == nil {
		log.Printf("clipboard pin skipped: store unavailable")
		return
	}
	ok := c.clipboard.Store().TogglePin(item.ID)
	if !ok {
		c.view.Status = c.view.Text("置顶失败", "Could not change the pin")
		log.Printf("clipboard pin failed: %v", c.clipboard.Store().LastError())
		c.refreshClipboardView()
		return
	}
	c.refreshClipboardView()
	if index := c.clipboard.Store().RowIndex(item.ID, c.view.Query, c.clipboardFilter); index >= 0 {
		c.view.ChooseClipboard(index)
	}
}

func (c *controller) deleteClipboardItem(item clipboard.Item) {
	if c.clipboard == nil {
		return
	}
	if !c.clipboard.Store().Delete(item.ID) {
		c.view.Status = c.view.Text("删除失败", "Could not delete the item")
		log.Printf("clipboard delete failed: %v", c.clipboard.Store().LastError())
		c.refreshClipboardView()
		return
	}
	c.refreshClipboardView()
}

// pasteToRememberedTarget writes already happened; this restores the original
// window and sends one bounded Ctrl+V, exactly like the launcher copy path.
func (c *controller) pasteToRememberedTarget() {
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
				c.view.Status = c.view.Text("粘贴未发送；内容已复制，可手动粘贴", "Paste was not sent; content is copied for manual paste")
				if errors.Is(err, windows.ErrPasteUnconfirmed) {
					c.view.Status = c.view.Text("已复制；粘贴状态未确认，请检查原窗口", "Copied; paste status is unconfirmed, please check the previous window")
				}
				log.Printf("clipboard paste canceled elapsed_us=%d error=%v", time.Since(started).Microseconds(), err)
				c.showPanel(false)
				return
			}
			c.view.Status = c.view.Text("已向原窗口发送粘贴", "Paste sent to the previous window")
			log.Printf("clipboard paste sent elapsed_us=%d", time.Since(started).Microseconds())
		})
	}()
}
