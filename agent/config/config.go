package config

import "os"

type Config struct {
	Port               string
	AgentURL           string
	JWTSecret          string
	GithubClientID     string
	GithubClientSecret string
	Role               string
	InternalSecret     string
	FollowerService    string
	DevFollowers       string
}

func Load() *Config {
	return &Config{
		Port:               getEnv("PORT", "8080"),
		AgentURL:           getEnv("AGENT_URL", "http://localhost:8080"),
		JWTSecret:          getEnv("JWT_SECRET", ""),
		GithubClientID:     getEnv("GITHUB_CLIENT_ID", ""),
		GithubClientSecret: getEnv("GITHUB_CLIENT_SECRET", ""),
		Role:               getEnv("DEVOPER_ROLE", "leader"),
		InternalSecret:     getEnv("DEVOPER_INTERNAL_SECRET", ""),
		FollowerService:    getEnv("DEVOPER_FOLLOWER_SERVICE", "devoper-agent"),
		DevFollowers:       getEnv("DEVOPER_DEV_FOLLOWERS", ""),
	}
}

func getEnv(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
