// Package icons supplies the launcher's embedded feature artwork. The glyphs
// follow the meanings of the Swift original's command and system symbols.
package icons

import (
	"embed"
	"strings"
	"sync"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/egoist/mygo/ui"
)

//go:embed assets/*.svg
var assets embed.FS

// Parse the bundled artwork once, keeping SVGs as vectors for every DPI.
var artwork = sync.OnceValue(func() map[string]*ui.SVG {
	files, err := assets.ReadDir("assets")
	if err != nil {
		panic(err)
	}
	images := make(map[string]*ui.SVG, len(files))
	for _, file := range files {
		data, err := assets.ReadFile("assets/" + file.Name())
		if err != nil {
			panic(err)
		}
		images[strings.TrimSuffix(file.Name(), ".svg")] = ui.MustParseSVG(data)
	}
	return images
})

// ForEntry resolves by stable identity, so localization, aliases and result
// order cannot change the glyph. User-authored entries use their kind's icon;
// installed applications keep their existing Windows shell artwork.
func ForEntry(entry domain.Application) *ui.SVG {
	var fallback string
	switch entry.Kind {
	case domain.KindCommand:
		fallback = "command"
	case domain.KindQuickAction:
		fallback = "command-ai-chat"
	case domain.KindSystemAction:
		fallback = "system"
	case domain.KindWindowCommand:
		fallback = "window"
	case domain.KindSystemSettings:
		fallback = "command-settings"
	case domain.KindQuicklink:
		return artwork()["quicklink"]
	case domain.KindSnippet:
		return artwork()["snippet"]
	default:
		return nil
	}
	images := artwork()
	if image := images[strings.ReplaceAll(entry.ID, ":", "-")]; image != nil {
		return image
	}
	return images[fallback]
}
