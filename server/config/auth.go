package config

import (
	"github.com/prvious/kontainer/server/app/facades"
)

func init() {
	config := facades.Config()
	config.Add("auth", map[string]any{
		"defaults": map[string]any{
			"guard": "user",
		},
		"guards": map[string]any{
			"user": map[string]any{
				"driver":   "jwt",
				"provider": "user",
				"ttl":      1440,
			},
		},
		"providers": map[string]any{
			"user": map[string]any{
				"driver": "orm",
			},
		},
	})
}
