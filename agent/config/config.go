package config

import "os"

type Config struct {
	Port               string
	AgentURL           string
	JWTSecret          string
	GithubClientID     string
	GithubClientSecret string
	AppScheme          string
}

func Load() *Config {
	return &Config{
		Port:               getEnv("PORT", "8080"),
		AgentURL:           getEnv("AGENT_URL", "http://localhost:8080"),
		JWTSecret:          getEnv("JWT_SECRET", ""),
		GithubClientID:     getEnv("GITHUB_CLIENT_ID", ""),
		GithubClientSecret: getEnv("GITHUB_CLIENT_SECRET", ""),
		AppScheme:          getEnv("APP_SCHEME", "devoper"),
	}
}

func getEnv(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
