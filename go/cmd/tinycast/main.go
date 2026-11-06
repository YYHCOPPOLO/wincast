package main

import (
	"log"
	"os"

	"github.com/YYHCOPPOLO/wincast/go/internal/app"
)

func main() {
	if err := app.Run(); err != nil {
		log.Print(err)
		os.Exit(1)
	}
}
