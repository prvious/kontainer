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
	MetricsInterval    string
	MetricsRetention   string
	MetricsDBPath      string
	GithubAllowedOrgs  string
	GithubAllowedUsers string
}

func Load() *Config {
	return &Config{
		Port:               getEnv("PORT", "8080"),
		AgentURL:           getEnv("AGENT_URL", "http://localhost:8080"),
		JWTSecret:          getEnv("JWT_SECRET", ""),
		GithubClientID:     getEnv("GITHUB_CLIENT_ID", ""),
		GithubClientSecret: getEnv("GITHUB_CLIENT_SECRET", ""),
		Role:               getEnv("KONTAINER_ROLE", "leader"),
		InternalSecret:     getEnv("KONTAINER_INTERNAL_SECRET", ""),
		FollowerService:    getEnv("KONTAINER_FOLLOWER_SERVICE", "kontainer-agent"),
		DevFollowers:       getEnv("KONTAINER_DEV_FOLLOWERS", ""),
		MetricsInterval:    getEnv("KONTAINER_METRICS_INTERVAL", "30s"),
		MetricsRetention:   getEnv("KONTAINER_METRICS_RETENTION", "24h"),
		MetricsDBPath:      getEnv("KONTAINER_METRICS_DB", "kontainer-metrics.db"),
		GithubAllowedOrgs:  getEnv("GITHUB_ALLOWED_ORGS", ""),
		GithubAllowedUsers: getEnv("GITHUB_ALLOWED_USERS", ""),
	}
}

func getEnv(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
