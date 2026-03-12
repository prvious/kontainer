package main

import (
	"github.com/prvious/kontainer/server/bootstrap"
)

func main() {
	app := bootstrap.Boot()

	app.Start()
}
