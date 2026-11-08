// Package brand embeds Wincast's application and notification-area artwork.
package brand

import (
	_ "embed"
	"sync"

	"github.com/egoist/mygo/ui"
)

//go:embed assets/icon.png
var icon []byte

//go:embed assets/tray.png
var Tray []byte

// IconBitmap decodes the application artwork once for native views.
var IconBitmap = sync.OnceValue(func() *ui.Bitmap {
	bitmap, _ := ui.DecodeBitmap(icon)
	return bitmap
})
