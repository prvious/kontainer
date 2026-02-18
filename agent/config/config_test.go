package config

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLoadFromYAML_Defaults(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(path, []byte(""), 0644)

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}

	if cfg.Port != "8080" {
		t.Errorf("Port = %q, want %q", cfg.Port, "8080")
	}
	if cfg.AgentURL != "http://localhost:8080" {
		t.Errorf("AgentURL = %q, want %q", cfg.AgentURL, "http://localhost:8080")
	}
	if cfg.Role != "leader" {
		t.Errorf("Role = %q, want %q", cfg.Role, "leader")
	}
	if cfg.Cluster.FollowerService != "kontainer-agent" {
		t.Errorf("FollowerService = %q, want %q", cfg.Cluster.FollowerService, "kontainer-agent")
	}
	if cfg.Metrics.Interval != "30s" {
		t.Errorf("Metrics.Interval = %q, want %q", cfg.Metrics.Interval, "30s")
	}
	if cfg.Metrics.Retention != "24h" {
		t.Errorf("Metrics.Retention = %q, want %q", cfg.Metrics.Retention, "24h")
	}
	if cfg.Metrics.DBPath != "kontainer-metrics.db" {
		t.Errorf("Metrics.DBPath = %q, want %q", cfg.Metrics.DBPath, "kontainer-metrics.db")
	}
	if cfg.Dev != false {
		t.Errorf("Dev = %v, want false", cfg.Dev)
	}
}

func TestLoadFromYAML_FullConfig(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "kontainer.yml")
	yaml := `
port: "9090"
agent_url: https://agent.example.com
role: follower
jwt_secret: my-jwt-secret
dev: true

github:
  client_id: gh-id
  client_secret: gh-secret
  allowed_users: [alice, bob]
  allowed_orgs: [myorg]

cluster:
  internal_secret: cluster-secret
  follower_service: my-followers
  dev_followers: ["localhost:8081", "localhost:8082"]

metrics:
  interval: 10s
  retention: 48h
  db_path: /data/metrics.db
`
	os.WriteFile(path, []byte(yaml), 0644)

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}

	if cfg.Port != "9090" {
		t.Errorf("Port = %q, want %q", cfg.Port, "9090")
	}
	if cfg.AgentURL != "https://agent.example.com" {
		t.Errorf("AgentURL = %q", cfg.AgentURL)
	}
	if cfg.Role != "follower" {
		t.Errorf("Role = %q", cfg.Role)
	}
	if cfg.JWTSecret != "my-jwt-secret" {
		t.Errorf("JWTSecret = %q", cfg.JWTSecret)
	}
	if cfg.Dev != true {
		t.Errorf("Dev = %v", cfg.Dev)
	}
	if cfg.GitHub.ClientID != "gh-id" {
		t.Errorf("GitHub.ClientID = %q", cfg.GitHub.ClientID)
	}
	if cfg.GitHub.ClientSecret != "gh-secret" {
		t.Errorf("GitHub.ClientSecret = %q", cfg.GitHub.ClientSecret)
	}
	if len(cfg.GitHub.AllowedUsers) != 2 || cfg.GitHub.AllowedUsers[0] != "alice" {
		t.Errorf("GitHub.AllowedUsers = %v", cfg.GitHub.AllowedUsers)
	}
	if len(cfg.GitHub.AllowedOrgs) != 1 || cfg.GitHub.AllowedOrgs[0] != "myorg" {
		t.Errorf("GitHub.AllowedOrgs = %v", cfg.GitHub.AllowedOrgs)
	}
	if cfg.Cluster.InternalSecret != "cluster-secret" {
		t.Errorf("Cluster.InternalSecret = %q", cfg.Cluster.InternalSecret)
	}
	if cfg.Cluster.FollowerService != "my-followers" {
		t.Errorf("Cluster.FollowerService = %q", cfg.Cluster.FollowerService)
	}
	if len(cfg.Cluster.DevFollowers) != 2 {
		t.Errorf("Cluster.DevFollowers = %v", cfg.Cluster.DevFollowers)
	}
	if cfg.Metrics.Interval != "10s" {
		t.Errorf("Metrics.Interval = %q", cfg.Metrics.Interval)
	}
	if cfg.Metrics.DBPath != "/data/metrics.db" {
		t.Errorf("Metrics.DBPath = %q", cfg.Metrics.DBPath)
	}
}

func TestResolveValue_FilePrefix(t *testing.T) {
	dir := t.TempDir()
	secretFile := filepath.Join(dir, "jwt-secret")
	os.WriteFile(secretFile, []byte("  file-secret-value\n"), 0644)

	result, err := resolveValue("file:" + secretFile)
	if err != nil {
		t.Fatalf("resolveValue error: %v", err)
	}
	if result != "file-secret-value" {
		t.Errorf("resolveValue = %q, want %q", result, "file-secret-value")
	}
}

func TestResolveValue_FilePrefix_MissingFile(t *testing.T) {
	_, err := resolveValue("file:/nonexistent/path/secret")
	if err == nil {
		t.Error("expected error for missing secret file")
	}
}

func TestResolveValue_EnvVar(t *testing.T) {
	t.Setenv("TEST_KONTAINER_SECRET", "env-secret-value")

	result, err := resolveValue("${TEST_KONTAINER_SECRET}")
	if err != nil {
		t.Fatalf("resolveValue error: %v", err)
	}
	if result != "env-secret-value" {
		t.Errorf("resolveValue = %q, want %q", result, "env-secret-value")
	}
}

func TestResolveValue_PlainLiteral(t *testing.T) {
	result, err := resolveValue("plain-value")
	if err != nil {
		t.Fatalf("resolveValue error: %v", err)
	}
	if result != "plain-value" {
		t.Errorf("resolveValue = %q, want %q", result, "plain-value")
	}
}

func TestLoad_FilePrefix_JWTSecret(t *testing.T) {
	dir := t.TempDir()
	secretFile := filepath.Join(dir, "jwt-secret")
	os.WriteFile(secretFile, []byte("  from-file\n"), 0644)

	configYAML := "jwt_secret: file:" + secretFile + "\n"
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}

	if cfg.JWTSecret != "from-file" {
		t.Errorf("JWTSecret = %q, want %q", cfg.JWTSecret, "from-file")
	}
}

func TestLoad_EnvVar_JWTSecret(t *testing.T) {
	t.Setenv("TEST_JWT_SECRET", "env-jwt-value")

	dir := t.TempDir()
	configYAML := "jwt_secret: ${TEST_JWT_SECRET}\n"
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}

	if cfg.JWTSecret != "env-jwt-value" {
		t.Errorf("JWTSecret = %q, want %q", cfg.JWTSecret, "env-jwt-value")
	}
}

func TestLoad_AllSecretFields_FilePrefix(t *testing.T) {
	dir := t.TempDir()

	writeSecret := func(name, value string) string {
		p := filepath.Join(dir, name)
		os.WriteFile(p, []byte(value), 0644)
		return p
	}

	jwtFile := writeSecret("jwt", "jwt-val")
	clientIDFile := writeSecret("client-id", "id-val")
	clientSecretFile := writeSecret("client-secret", "secret-val")
	internalFile := writeSecret("internal", "internal-val")

	configYAML := `
jwt_secret: file:` + jwtFile + `
github:
  client_id: file:` + clientIDFile + `
  client_secret: file:` + clientSecretFile + `
cluster:
  internal_secret: file:` + internalFile + `
`
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}

	if cfg.JWTSecret != "jwt-val" {
		t.Errorf("JWTSecret = %q", cfg.JWTSecret)
	}
	if cfg.GitHub.ClientID != "id-val" {
		t.Errorf("GitHub.ClientID = %q", cfg.GitHub.ClientID)
	}
	if cfg.GitHub.ClientSecret != "secret-val" {
		t.Errorf("GitHub.ClientSecret = %q", cfg.GitHub.ClientSecret)
	}
	if cfg.Cluster.InternalSecret != "internal-val" {
		t.Errorf("Cluster.InternalSecret = %q", cfg.Cluster.InternalSecret)
	}
}

func TestResolveValue_UnsetEnvVar_Error(t *testing.T) {
	_, err := resolveValue("${TOTALLY_UNSET_VAR_FOR_TEST}")
	if err == nil {
		t.Error("expected error for unset env var")
	}
	if !strings.Contains(err.Error(), "TOTALLY_UNSET_VAR_FOR_TEST") {
		t.Errorf("error should mention the missing var, got: %v", err)
	}
}

func TestLoad_UnsetEnvVar_JWTSecret_Error(t *testing.T) {
	dir := t.TempDir()
	configYAML := "jwt_secret: ${TOTALLY_UNSET_JWT_SECRET_FOR_TEST}\n"
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	_, err := Load(configPath)
	if err == nil {
		t.Error("expected error for unset env var in jwt_secret")
	}
}

func TestLoad_FilePrefix_MissingFile_Error(t *testing.T) {
	dir := t.TempDir()
	configYAML := "jwt_secret: file:/nonexistent/path/secret\n"
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	_, err := Load(configPath)
	if err == nil {
		t.Error("expected error for missing secret file")
	}
}

func TestLoad_FileNotFound(t *testing.T) {
	_, err := Load("/nonexistent/kontainer.yml")
	if err == nil {
		t.Error("expected error for missing config file")
	}
}
