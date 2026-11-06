package app

import (
	"bytes"
	"image"
	"image/color"
	"image/png"
	"math"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

type iconKey struct {
	path   string
	pixels int
}

func (c *controller) icon(application domain.Application) *ui.Bitmap {
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
	image := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	ink := color.NRGBA{R: 129, G: 140, B: 248, A: 255}
	for y := 4; y < 28; y++ {
		for x := 4; x < 28; x++ {
			if (y >= 5 && y <= 9) || (x >= 13 && x <= 17) {
				image.SetNRGBA(x, y, ink)
			}
		}
	}
	var buffer bytes.Buffer
	_ = png.Encode(&buffer, image)
	return buffer.Bytes()
}
