package app

import (
	"math"

	"github.com/YYHCOPPOLO/wincast/go/internal/brand"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

type iconKey struct {
	path   string
	pixels int
}

func (c *controller) icon(application domain.Application) *ui.Bitmap {
	if application.Path == "" {
		return nil
	}
	key := iconKey{application.Path, int(math.Ceil(28 * c.scale))}
	if bitmap, ok := c.icons[key]; ok {
		return bitmap
	}
	if !c.pendingIcons[key] {
		select {
		case c.iconRequests <- key:
			c.pendingIcons[key] = true
		default:
		}
	}
	return nil
}

func (c *controller) startIcons() {
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		for {
			select {
			case <-c.ctx.Done():
				return
			case key := <-c.iconRequests:
				if c.ctx.Err() != nil {
					return
				}
				data, err := mygo.Shell.FileIcon(key.path, key.pixels)
				var bitmap *ui.Bitmap
				if err == nil {
					bitmap, _ = ui.DecodeBitmap(data)
				}
				if c.ctx.Err() != nil {
					return
				}
				c.panel.Update(func() {
					if c.closing {
						return
					}
					delete(c.pendingIcons, key)
					if len(c.icons) >= 256 {
						for old := range c.icons {
							delete(c.icons, old)
							break
						}
					}
					c.icons[key] = bitmap
				})
			}
		}
	}()
}

func trayIcon() []byte {
	return brand.Tray
}
