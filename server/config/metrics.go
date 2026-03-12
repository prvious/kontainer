package config

import (
	"github.com/prvious/kontainer/server/app/facades"
)

func init() {
	config := facades.Config()
	config.Add("metrics", map[string]any{
		"interval":  config.Env("METRICS_INTERVAL", 30),
		"retention": config.Env("METRICS_RETENTION", 1440),
	})
}
