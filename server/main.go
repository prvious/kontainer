package main

import (
	"log"

	"github.com/prvious/kontainer/server/app/helpers"
	"github.com/prvious/kontainer/server/bootstrap"
)

func main() {
	if err := helpers.ResolveFileEnvVars(
		"CLUSTER_SECRET",
		"JWT_SECRET",
		"DB_DATABASE",
	); err != nil {
		log.Fatalf("Failed to resolve env file vars: %v", err)
	}

	app := bootstrap.Boot()

	app.Start()
}
