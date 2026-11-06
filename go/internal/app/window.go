package app

import (
	"log"

	"github.com/egoist/mygo"
)

func showWindow(window *mygo.Window) {
	window.Show()
	// STARTUPINFO may request SW_HIDE, overriding the first ShowWindow even
	// when the app explicitly requested Show. A hidden-created panel has not
	// consumed that override yet. Retry once based on actual native visibility.
	if !window.IsVisible() {
		window.Show()
		log.Printf("window show retried visible=%t", window.IsVisible())
	}
	window.Focus()
}
