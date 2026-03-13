package config

import (
	"github.com/prvious/kontainer/server/app/facades"
)

func init() {
	config := facades.Config()
	config.Add("jwt", map[string]any{
		"secret":      config.Env("JWT_SECRET", ""),
		"ttl":         1440,
		"refresh_ttl": 0,
	})
}
