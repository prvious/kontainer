package config

import (
	"fmt"
	"os"
	"strings"

	"gopkg.in/yaml.v3"
)

type GitHubConfig struct {
	ClientID     string   `yaml:"client_id"`
	ClientSecret string   `yaml:"client_secret"`
	AllowedUsers []string `yaml:"allowed_users"`
	AllowedOrgs  []string `yaml:"allowed_orgs"`
}

type ClusterConfig struct {
	InternalSecret  string   `yaml:"internal_secret"`
	FollowerService string   `yaml:"follower_service"`
	DevFollowers    []string `yaml:"dev_followers"`
}

type MetricsConfig struct {
	Interval  string `yaml:"interval"`
	Retention string `yaml:"retention"`
	DBPath    string `yaml:"db_path"`
}

type Config struct {
	Port     string `yaml:"port"`
	AgentURL string `yaml:"agent_url"`
	Role     string `yaml:"role"`

	JWTSecret string `yaml:"jwt_secret"`

	GitHub  GitHubConfig  `yaml:"github"`
	Cluster ClusterConfig `yaml:"cluster"`
	Metrics MetricsConfig `yaml:"metrics"`
	Dev     bool          `yaml:"dev"`
}

// Load reads a YAML config file and returns a Config with defaults applied
// and secret references resolved (file: prefix and ${ENV_VAR} expansion).
func Load(path string) (*Config, error) {
	cfg := &Config{
		Port:     "8080",
		AgentURL: "http://localhost:8080",
		Role:     "leader",
		Cluster: ClusterConfig{
			FollowerService: "kontainer-agent",
		},
		Metrics: MetricsConfig{
			Interval:  "30s",
			Retention: "24h",
			DBPath:    "kontainer-metrics.db",
		},
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("reading config file: %w", err)
	}

	if err := yaml.Unmarshal(data, cfg); err != nil {
		return nil, fmt.Errorf("parsing config file: %w", err)
	}

	if err := resolveSecrets(cfg); err != nil {
		return nil, err
	}

	return cfg, nil
}

// resolveValue resolves a config string value using one of three methods:
//   - "file:/path/to/file" → reads the file and trims whitespace
//   - "${ENV_VAR}" → expands environment variables via os.ExpandEnv
//   - "plain-value" → returned as-is
func resolveValue(val string) (string, error) {
	if strings.HasPrefix(val, "file:") {
		data, err := os.ReadFile(strings.TrimPrefix(val, "file:"))
		if err != nil {
			return "", fmt.Errorf("reading secret from %q: %w", val, err)
		}
		return strings.TrimSpace(string(data)), nil
	}
	return os.ExpandEnv(val), nil
}

// resolveSecrets calls resolveValue on all sensitive string fields.
func resolveSecrets(cfg *Config) error {
	var err error

	if cfg.JWTSecret, err = resolveValue(cfg.JWTSecret); err != nil {
		return fmt.Errorf("jwt_secret: %w", err)
	}
	if cfg.GitHub.ClientID, err = resolveValue(cfg.GitHub.ClientID); err != nil {
		return fmt.Errorf("github.client_id: %w", err)
	}
	if cfg.GitHub.ClientSecret, err = resolveValue(cfg.GitHub.ClientSecret); err != nil {
		return fmt.Errorf("github.client_secret: %w", err)
	}
	if cfg.Cluster.InternalSecret, err = resolveValue(cfg.Cluster.InternalSecret); err != nil {
		return fmt.Errorf("cluster.internal_secret: %w", err)
	}

	return nil
}
