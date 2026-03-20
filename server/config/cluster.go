package config

import (
	"github.com/prvious/kontainer/server/app/facades"
)

func init() {
	config := facades.Config()

	role, _ := config.Env("AGENT_ROLE", "leader").(string)
	if role != "leader" && role != "follower" {
		role = "leader"
	}

	config.Add("cluster", map[string]any{
		"role":             role,
		"secret":           config.Env("CLUSTER_SECRET", ""),
		"follower_service": config.Env("CLUSTER_FOLLOWER_SERVICE", "kontainer-agent"),
		"dev_followers":    config.Env("CLUSTER_DEV_FOLLOWERS", ""),
		"agent_url":        config.Env("AGENT_URL", "http://localhost:3000"),
	})
}
