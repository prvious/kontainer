package main

import (
	"github.com/prvious/kontainer/server/app/helpers"
	"github.com/prvious/kontainer/server/bootstrap"
)

func main() {
	helpers.ResolveFileEnvVars(
		"CLUSTER_SECRET",
		"JWT_SECRET",
		"DB_DATABASE",
	)

	app := bootstrap.Boot()

	app.Start()
}
