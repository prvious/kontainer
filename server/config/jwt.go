package config

import (
	"github.com/prvious/kontainer/server/app/facades"
)

func init() {
	config := facades.Config()
	config.Add("jwt", map[string]any{
		"secret":      config.Env("JWT_SECRET", ""),
		"ttl":         config.Env("JWT_TTL", 1440),
		"refresh_ttl": config.Env("JWT_REFRESH_TTL", 0),
	})
}
