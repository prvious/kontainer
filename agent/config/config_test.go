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
dev: true

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
	if cfg.Dev != true {
		t.Errorf("Dev = %v", cfg.Dev)
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

func TestLoad_FilePrefix_InternalSecret(t *testing.T) {
	dir := t.TempDir()
	secretFile := filepath.Join(dir, "internal-secret")
	os.WriteFile(secretFile, []byte("  from-file\n"), 0644)

	configYAML := "cluster:\n  internal_secret: file:" + secretFile + "\n"
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}

	if cfg.Cluster.InternalSecret != "from-file" {
		t.Errorf("Cluster.InternalSecret = %q, want %q", cfg.Cluster.InternalSecret, "from-file")
	}
}

func TestLoad_EnvVar_InternalSecret(t *testing.T) {
	t.Setenv("TEST_INTERNAL_SECRET", "env-internal-value")

	dir := t.TempDir()
	configYAML := "cluster:\n  internal_secret: ${TEST_INTERNAL_SECRET}\n"
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}

	if cfg.Cluster.InternalSecret != "env-internal-value" {
		t.Errorf("Cluster.InternalSecret = %q, want %q", cfg.Cluster.InternalSecret, "env-internal-value")
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

func TestLoad_UnsetEnvVar_InternalSecret_Error(t *testing.T) {
	dir := t.TempDir()
	configYAML := "cluster:\n  internal_secret: ${TOTALLY_UNSET_INTERNAL_SECRET_FOR_TEST}\n"
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	_, err := Load(configPath)
	if err == nil {
		t.Error("expected error for unset env var in internal_secret")
	}
}

func TestLoad_FilePrefix_MissingFile_Error(t *testing.T) {
	dir := t.TempDir()
	configYAML := "cluster:\n  internal_secret: file:/nonexistent/path/secret\n"
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
