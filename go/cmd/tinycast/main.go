package main

import (
	"log"
	"os"

	"github.com/YYHCOPPOLO/wincast/go/internal/app"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

func main() {
	if err := app.Run(); err != nil {
		log.Print(err)
		windows.ShowStartupError(err.Error())
		os.Exit(1)
	}
}
