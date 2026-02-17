# YAML-Only Agent Configuration — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace all environment variable configuration in the Go agent with a single `kontainer.yml` YAML config file.

**Architecture:** Rewrite `config/config.go` to parse YAML with `gopkg.in/yaml.v3`, resolve `_file` secret references, and validate at startup. Update `main.go` to accept a `--config` flag (default: `./kontainer.yml`). Remove `godotenv` dependency entirely. Thread `Config.Dev` into the metrics action instead of reading `KONTAINER_DEV` from env.

**Tech Stack:** Go stdlib (`flag`, `os`, `strings`), `gopkg.in/yaml.v3`

**Design doc:** `docs/plans/2026-02-17-yaml-config-design.md`

---

### Task 1: Add yaml.v3, remove godotenv

**Files:**
- Modify: `agent/go.mod`

**Step 1: Add yaml.v3 and remove godotenv**

Run:
```bash
go -C agent get gopkg.in/yaml.v3
go -C agent get -u github.com/joho/godotenv@none
go -C agent mod tidy
```

**Step 2: Verify**

Run: `go -C agent build ./...`
Expected: Build fails (godotenv import still in main.go) — that's fine, we'll fix it in Task 3.

**Step 3: Commit**

```bash
git add agent/go.mod agent/go.sum
git commit -m "chore: add gopkg.in/yaml.v3, remove godotenv dependency"
```

---

### Task 2: Rewrite config/config.go with YAML parsing

**Files:**
- Rewrite: `agent/config/config.go`
- Create: `agent/config/config_test.go`

**Step 1: Write the tests**

Create `agent/config/config_test.go`:

```go
package config

import (
	"os"
	"path/filepath"
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

func TestResolveSecretFromFile(t *testing.T) {
	dir := t.TempDir()
	secretFile := filepath.Join(dir, "jwt-secret")
	os.WriteFile(secretFile, []byte("  file-secret-value\n"), 0644)

	configYAML := `jwt_secret_file: ` + secretFile + "\n"
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}

	if cfg.JWTSecret != "file-secret-value" {
		t.Errorf("JWTSecret = %q, want %q", cfg.JWTSecret, "file-secret-value")
	}
}

func TestResolveSecretFromFile_PrecedenceOverInline(t *testing.T) {
	dir := t.TempDir()
	secretFile := filepath.Join(dir, "jwt-secret")
	os.WriteFile(secretFile, []byte("from-file"), 0644)

	configYAML := "jwt_secret: inline-value\njwt_secret_file: " + secretFile + "\n"
	configPath := filepath.Join(dir, "kontainer.yml")
	os.WriteFile(configPath, []byte(configYAML), 0644)

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}

	if cfg.JWTSecret != "from-file" {
		t.Errorf("JWTSecret = %q, want %q (file should take precedence)", cfg.JWTSecret, "from-file")
	}
}

func TestResolveSecretFromFile_AllSecretFields(t *testing.T) {
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
jwt_secret_file: ` + jwtFile + `
github:
  client_id_file: ` + clientIDFile + `
  client_secret_file: ` + clientSecretFile + `
cluster:
  internal_secret_file: ` + internalFile + `
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

func TestResolveSecretFromFile_MissingFile(t *testing.T) {
	dir := t.TempDir()
	configYAML := "jwt_secret_file: /nonexistent/path/secret\n"
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
```

**Step 2: Run tests to verify they fail**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./config/...`
Expected: FAIL — `Load` signature doesn't match yet.

**Step 3: Rewrite config.go**

Replace `agent/config/config.go` with:

```go
package config

import (
	"fmt"
	"os"
	"strings"

	"gopkg.in/yaml.v3"
)

type GitHubConfig struct {
	ClientID         string   `yaml:"client_id"`
	ClientIDFile     string   `yaml:"client_id_file"`
	ClientSecret     string   `yaml:"client_secret"`
	ClientSecretFile string   `yaml:"client_secret_file"`
	AllowedUsers     []string `yaml:"allowed_users"`
	AllowedOrgs      []string `yaml:"allowed_orgs"`
}

type ClusterConfig struct {
	InternalSecret     string   `yaml:"internal_secret"`
	InternalSecretFile string   `yaml:"internal_secret_file"`
	FollowerService    string   `yaml:"follower_service"`
	DevFollowers       []string `yaml:"dev_followers"`
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

	JWTSecret     string `yaml:"jwt_secret"`
	JWTSecretFile string `yaml:"jwt_secret_file"`

	GitHub  GitHubConfig  `yaml:"github"`
	Cluster ClusterConfig `yaml:"cluster"`
	Metrics MetricsConfig `yaml:"metrics"`
	Dev     bool          `yaml:"dev"`
}

// Load reads a YAML config file and returns a Config with defaults applied
// and _file secret references resolved.
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

// resolveSecrets reads _file references and populates the corresponding
// secret fields. File content takes precedence over inline values.
func resolveSecrets(cfg *Config) error {
	resolve := func(value *string, filePath string, name string) error {
		if filePath == "" {
			return nil
		}
		data, err := os.ReadFile(filePath)
		if err != nil {
			return fmt.Errorf("reading %s from %q: %w", name, filePath, err)
		}
		*value = strings.TrimSpace(string(data))
		return nil
	}

	if err := resolve(&cfg.JWTSecret, cfg.JWTSecretFile, "jwt_secret_file"); err != nil {
		return err
	}
	if err := resolve(&cfg.GitHub.ClientID, cfg.GitHub.ClientIDFile, "github.client_id_file"); err != nil {
		return err
	}
	if err := resolve(&cfg.GitHub.ClientSecret, cfg.GitHub.ClientSecretFile, "github.client_secret_file"); err != nil {
		return err
	}
	if err := resolve(&cfg.Cluster.InternalSecret, cfg.Cluster.InternalSecretFile, "cluster.internal_secret_file"); err != nil {
		return err
	}

	return nil
}
```

**Step 4: Run tests to verify they pass**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./config/... -v`
Expected: All PASS.

**Step 5: Commit**

```bash
git add agent/config/config.go agent/config/config_test.go
git commit -m "feat: rewrite config to load from YAML with _file secret resolution"
```

---

### Task 3: Update main.go to use YAML config loading

**Files:**
- Modify: `agent/main.go`

**Step 1: Update main.go**

Changes needed:
1. Remove `godotenv` import
2. Add `flag` import
3. Replace `config.Load()` with `config.Load(configPath)` using `--config` flag
4. Remove comma-split parsing for `allowedOrgs`, `allowedUsers` — use `cfg.GitHub.AllowedOrgs` and `cfg.GitHub.AllowedUsers` directly (already slices)
5. Replace `cluster.ParseDevFollowers(cfg.DevFollowers)` with `cfg.Cluster.DevFollowers` (already a slice)
6. Remove `os.Getenv("DOCKER_NODE_ID")` reads (both in startFollower and startLeader) — keep only auto-detection from Docker
7. Update all `cfg.FieldName` references to new struct paths (e.g. `cfg.InternalSecret` → `cfg.Cluster.InternalSecret`)
8. Update fatal message from `"KONTAINER_INTERNAL_SECRET is required"` to `"cluster.internal_secret is required for follower mode"`

The full updated `main.go`:

```go
package main

import (
	"context"
	"flag"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/prvious/kontainer/agent/auth"
	"github.com/prvious/kontainer/agent/auth/providers"
	"github.com/prvious/kontainer/agent/cluster"
	"github.com/prvious/kontainer/agent/config"
	"github.com/prvious/kontainer/agent/images"
	"github.com/prvious/kontainer/agent/metrics"
	"github.com/prvious/kontainer/agent/server"
	"github.com/prvious/kontainer/agent/services"
	"github.com/prvious/kontainer/agent/settings"
	"github.com/prvious/kontainer/agent/storage"
	"github.com/docker/docker/client"
)

func mask(s string) string {
	if len(s) <= 4 {
		return "***"
	}
	return s[:4] + "***"
}

func main() {
	configPath := flag.String("config", "kontainer.yml", "path to config file")
	flag.Parse()

	cfg, err := config.Load(*configPath)
	if err != nil {
		log.Fatalf("failed to load config: %v", err)
	}

	log.Println("--- kontainer agent config ---")
	log.Printf("  config file     : %s", *configPath)
	log.Printf("  role            : %s", cfg.Role)
	log.Printf("  agent url       : %s", cfg.AgentURL)
	log.Printf("  github client id: %s", mask(cfg.GitHub.ClientID))
	log.Printf("  jwt secret      : %s", mask(cfg.JWTSecret))

	if cfg.Role == cluster.RoleFollower {
		startFollower(cfg)
	} else {
		startLeader(cfg)
	}
}

func startFollower(cfg *config.Config) {
	if cfg.Cluster.InternalSecret == "" {
		log.Fatal("cluster.internal_secret is required for follower mode")
	}

	hostname, _ := os.Hostname()

	dockerClient, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		log.Fatalf("Failed to create Docker client: %v", err)
	}

	nodeID := ""
	info, err := dockerClient.Info(context.Background())
	if err == nil && info.Swarm.NodeID != "" {
		nodeID = info.Swarm.NodeID
	}

	nodeAction := metrics.NewCollectNodeMetricsAction(dockerClient, cfg.Dev)
	containerAction := metrics.NewCollectContainerMetricsAction(dockerClient)
	metricsHandler := metrics.NewInternalMetricsHandler(nodeAction, containerAction, nodeID, hostname)

	identity := server.FollowerIdentity{
		NodeID:   nodeID,
		Hostname: hostname,
	}

	listImagesAction := images.NewListImagesAction(dockerClient)
	internalImagesHandler := images.NewInternalImagesHandler(listImagesAction, nodeID, hostname)
	internalRemoveImageHandler := images.NewInternalRemoveImageHandler(dockerClient)
	internalPruneImagesHandler := images.NewInternalPruneImagesHandler(dockerClient)

	execFollowerHandler := services.NewExecContainerFollowerHandler(dockerClient)
	handler := server.NewFollower(identity, cfg.Cluster.InternalSecret, metricsHandler, execFollowerHandler, internalImagesHandler, internalRemoveImageHandler, internalPruneImagesHandler)

	srv := &http.Server{Addr: ":" + cfg.Port, Handler: handler}

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigCh
		log.Println("shutting down...")
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		srv.Shutdown(ctx)
	}()

	log.Printf("  listening on    : %s (follower)", srv.Addr)
	log.Println("----------------------------")

	if err := srv.ListenAndServe(); err != http.ErrServerClosed {
		log.Fatalf("server error: %v", err)
	}
}

func startLeader(cfg *config.Config) {
	if cfg.GitHub.ClientID == "" || cfg.GitHub.ClientSecret == "" {
		log.Println("  ⚠ github.client_id or github.client_secret is empty — OAuth will not work")
	}
	if cfg.JWTSecret == "" {
		log.Println("  ⚠ jwt_secret is empty — auth tokens will not be signed")
	}

	if len(cfg.GitHub.AllowedOrgs) > 0 {
		log.Printf("  GitHub org allowlist: %v", cfg.GitHub.AllowedOrgs)
	}
	if len(cfg.GitHub.AllowedUsers) > 0 {
		log.Printf("  GitHub user allowlist: %v", cfg.GitHub.AllowedUsers)
	}

	registry := providers.NewProviderRegistry()
	registry.Register(providers.NewGitHubProvider(
		cfg.GitHub.ClientID,
		cfg.GitHub.ClientSecret,
		cfg.AgentURL+"/auth/github/callback",
		cfg.GitHub.AllowedOrgs,
		cfg.GitHub.AllowedUsers,
	))

	dockerClient, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		log.Fatalf("Failed to create Docker client: %v", err)
	}

	hostname, _ := os.Hostname()

	nodeAction := metrics.NewCollectNodeMetricsAction(dockerClient, cfg.Dev)
	containerAction := metrics.NewCollectContainerMetricsAction(dockerClient)
	leaderNodeID := ""
	info, err := dockerClient.Info(context.Background())
	if err == nil && info.Swarm.NodeID != "" {
		leaderNodeID = info.Swarm.NodeID
	}
	internalMetricsHandler := metrics.NewInternalMetricsHandler(nodeAction, containerAction, leaderNodeID, hostname)

	listServicesAction := services.NewListServicesAction(dockerClient)
	listServicesHandler := services.NewListServicesHandler(listServicesAction)

	inspectServiceAction := services.NewInspectServiceAction(dockerClient)
	inspectServiceHandler := services.NewInspectServiceHandler(inspectServiceAction)

	scaleServiceAction := services.NewScaleServiceAction(dockerClient)
	scaleServiceHandler := services.NewScaleServiceHandler(scaleServiceAction)

	restartServiceAction := services.NewRestartServiceAction(dockerClient)
	restartServiceHandler := services.NewRestartServiceHandler(restartServiceAction)

	rollbackServiceAction := services.NewRollbackServiceAction(dockerClient)
	rollbackServiceHandler := services.NewRollbackServiceHandler(rollbackServiceAction)

	listServiceTasksAction := services.NewListServiceTasksAction(dockerClient)
	listServiceTasksHandler := services.NewListServiceTasksHandler(listServiceTasksAction)

	inspectTaskAction := services.NewInspectTaskAction(dockerClient)
	inspectTaskHandler := services.NewInspectTaskHandler(inspectTaskAction)

	listStacksAction := services.NewListStacksAction(dockerClient)
	listStacksHandler := services.NewListStacksHandler(listStacksAction)

	removeStackAction := services.NewRemoveStackAction(dockerClient)
	removeStackHandler := services.NewRemoveStackHandler(removeStackAction)

	listNodesAction := services.NewListNodesAction(dockerClient)
	listNodesHandler := services.NewListNodesHandler(listNodesAction)

	listNodeTasksAction := services.NewListNodeTasksAction(dockerClient)
	listNodeTasksHandler := services.NewListNodeTasksHandler(listNodeTasksAction)

	listTaskStatsAction := services.NewListTaskStatsAction(dockerClient)
	listTaskStatsHandler := services.NewListTaskStatsHandler(listTaskStatsAction)

	listTasksAction := services.NewListTasksAction(dockerClient)
	listTasksHandler := services.NewListTasksHandler(listTasksAction)

	listNetworksAction := services.NewListNetworksAction(dockerClient)
	listNetworksHandler := services.NewListNetworksHandler(listNetworksAction)

	listVolumesAction := services.NewListVolumesAction(dockerClient)
	listVolumesHandler := services.NewListVolumesHandler(listVolumesAction)

	updateServiceEnvAction := services.NewUpdateServiceEnvAction(dockerClient)
	updateServiceEnvHandler := services.NewUpdateServiceEnvHandler(updateServiceEnvAction)

	updateServiceImageAction := services.NewUpdateServiceImageAction(dockerClient)
	updateServiceImageHandler := services.NewUpdateServiceImageHandler(updateServiceImageAction)

	updateNodeAvailabilityAction := services.NewUpdateNodeAvailabilityAction(dockerClient)
	updateNodeAvailabilityHandler := services.NewUpdateNodeAvailabilityHandler(updateNodeAvailabilityAction)

	updateNodeRoleAction := services.NewUpdateNodeRoleAction(dockerClient)
	updateNodeRoleHandler := services.NewUpdateNodeRoleHandler(updateNodeRoleAction)

	listSecretsAction := services.NewListSecretsAction(dockerClient)
	listSecretsHandler := services.NewListSecretsHandler(listSecretsAction)

	createSecretAction := services.NewCreateSecretAction(dockerClient)
	createSecretHandler := services.NewCreateSecretHandler(createSecretAction)

	removeSecretAction := services.NewRemoveSecretAction(dockerClient)
	removeSecretHandler := services.NewRemoveSecretHandler(removeSecretAction)

	listConfigsAction := services.NewListConfigsAction(dockerClient)
	listConfigsHandler := services.NewListConfigsHandler(listConfigsAction)

	createConfigAction := services.NewCreateConfigAction(dockerClient)
	createConfigHandler := services.NewCreateConfigHandler(createConfigAction)

	removeConfigAction := services.NewRemoveConfigAction(dockerClient)
	removeConfigHandler := services.NewRemoveConfigHandler(removeConfigAction)

	inspectConfigAction := services.NewInspectConfigAction(dockerClient)
	inspectConfigHandler := services.NewInspectConfigHandler(inspectConfigAction)

	streamLogsHandler := services.NewStreamServiceLogsHandler(dockerClient, cfg.JWTSecret)
	streamTaskLogsHandler := services.NewStreamTaskLogsHandler(dockerClient, cfg.JWTSecret)

	sessions := auth.NewSessionStore()
	bridge := auth.NewAuthBridge()
	authHandler := auth.NewAuthHandler(registry, sessions, cfg.JWTSecret, bridge)
	jwtMiddleware := auth.JWTMiddleware(cfg.JWTSecret)

	clusterRegistry := cluster.NewRegistry()
	selfInfo := cluster.NewAgentInfo("", hostname, cfg.AgentURL, cluster.RoleLeader, "healthy")

	listAgentsAction := services.NewListAgentsAction(clusterRegistry, selfInfo)
	listAgentsHandler := services.NewListAgentsHandler(listAgentsAction)

	execHandler := services.NewExecContainerHandler(dockerClient, cfg.JWTSecret, clusterRegistry, leaderNodeID, cfg.Cluster.InternalSecret)

	interval, err := time.ParseDuration(cfg.Metrics.Interval)
	if err != nil {
		interval = 30 * time.Second
	}
	retention, err := time.ParseDuration(cfg.Metrics.Retention)
	if err != nil {
		retention = 24 * time.Hour
	}

	metricsStore, err := storage.NewMetricsStore(cfg.Metrics.DBPath, retention)
	if err != nil {
		log.Fatalf("Failed to create metrics store: %v", err)
	}

	settingsStore, err := storage.NewSettingsStore(cfg.Metrics.DBPath)
	if err != nil {
		log.Fatalf("Failed to create settings store: %v", err)
	}

	collector := metrics.NewMetricsCollector(internalMetricsHandler, clusterRegistry, cfg.Cluster.InternalSecret, metricsStore, interval)
	collector.Start(context.Background())

	clusterMetricsHandler := metrics.NewClusterMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.Cluster.InternalSecret, metricsStore)
	nodeMetricsHandler := metrics.NewNodeMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.Cluster.InternalSecret)

	listImagesAction := images.NewListImagesAction(dockerClient)
	leaderImagesHandler := images.NewInternalImagesHandler(listImagesAction, leaderNodeID, hostname)
	clusterImagesHandler := images.NewClusterImagesHandler(leaderImagesHandler, clusterRegistry, cfg.Cluster.InternalSecret)
	removeImageHandler := images.NewRemoveImageHandler(dockerClient, clusterRegistry, leaderNodeID, cfg.Cluster.InternalSecret)
	pruneImagesHandler := images.NewPruneImagesHandler(dockerClient, clusterRegistry, leaderNodeID, cfg.Cluster.InternalSecret)

	getSettingsAction := settings.NewGetSettingsAction(settingsStore)
	getSettingsHandler := settings.NewGetSettingsHandler(getSettingsAction)
	updateSettingsAction := settings.NewUpdateSettingsAction(settingsStore)
	updateSettingsHandler := settings.NewUpdateSettingsHandler(updateSettingsAction, getSettingsAction)

	if len(cfg.Cluster.DevFollowers) > 0 {
		log.Printf("  dev followers   : %v", cfg.Cluster.DevFollowers)
		devDiscovery := cluster.NewDevDiscovery(cfg.Cluster.DevFollowers, cfg.Cluster.InternalSecret, clusterRegistry)
		devDiscovery.Start(context.Background())
		defer devDiscovery.Stop()
	} else if cfg.Cluster.InternalSecret != "" {
		discovery := cluster.NewDiscovery(dockerClient, clusterRegistry, cfg.Cluster.FollowerService, cfg.Cluster.InternalSecret, cfg.Port)
		discovery.Start(context.Background())
		defer discovery.Stop()
	}

	handler := server.New(authHandler, jwtMiddleware, listServicesHandler, inspectServiceHandler, scaleServiceHandler, restartServiceHandler, rollbackServiceHandler, listStacksHandler, removeStackHandler, listServiceTasksHandler, inspectTaskHandler, listNodesHandler, listNodeTasksHandler, listTaskStatsHandler, listTasksHandler, streamLogsHandler, streamTaskLogsHandler, listNetworksHandler, listVolumesHandler, updateServiceEnvHandler, updateServiceImageHandler, updateNodeAvailabilityHandler, updateNodeRoleHandler, listSecretsHandler, createSecretHandler, removeSecretHandler, listConfigsHandler, createConfigHandler, removeConfigHandler, inspectConfigHandler, listAgentsHandler, execHandler, clusterMetricsHandler, nodeMetricsHandler, clusterImagesHandler, removeImageHandler, pruneImagesHandler, getSettingsHandler, updateSettingsHandler)

	srv := &http.Server{Addr: ":" + cfg.Port, Handler: handler}

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigCh
		log.Println("shutting down...")
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		srv.Shutdown(ctx)
	}()

	log.Printf("  listening on    : %s (leader)", srv.Addr)
	log.Println("----------------------------")

	if err := srv.ListenAndServe(); err != http.ErrServerClosed {
		log.Fatalf("server error: %v", err)
	}

	collector.Stop()
	settingsStore.Close()
	metricsStore.Close()
}
```

**Step 2: Verify build**

Run: `go -C /Users/clovismuneza/Apps/kontainer/agent build ./...`
Expected: Fails — `NewCollectNodeMetricsAction` signature doesn't match yet (needs `dev bool` param). We'll fix that in Task 4.

**Step 3: Commit**

```bash
git add agent/main.go
git commit -m "feat: update main.go to load YAML config with --config flag"
```

---

### Task 4: Thread Dev flag into CollectNodeMetricsAction

**Files:**
- Modify: `agent/metrics/collect_node_metrics.go`
- Modify: `agent/metrics/collect_node_metrics_test.go`

**Step 1: Update the test first**

Replace the dev mode test in `collect_node_metrics_test.go`:

```go
func TestCollectNodeMetrics_DevMode(t *testing.T) {
	action := NewCollectNodeMetricsAction(nil, true)
	_, err := action.Execute(context.Background())
	if err == nil {
		t.Error("expected error with nil docker client in dev mode")
	}
}
```

Also add a test for non-dev mode:

```go
func TestCollectNodeMetrics_NonDevMode(t *testing.T) {
	action := NewCollectNodeMetricsAction(nil, false)
	m, err := action.Execute(context.Background())
	if err != nil {
		t.Fatalf("non-dev mode should use /proc, got error: %v", err)
	}
	if m.CPUs <= 0 {
		t.Errorf("expected CPUs > 0, got %d", m.CPUs)
	}
}
```

**Step 2: Run tests to verify they fail**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./metrics/... -v`
Expected: FAIL — constructor signature doesn't match.

**Step 3: Update collect_node_metrics.go**

Changes:
- Add `devMode bool` field to struct
- Accept `devMode bool` in `NewCollectNodeMetricsAction`
- Replace `os.Getenv("KONTAINER_DEV") == "true"` with `a.devMode`
- Remove `"os"` from imports (only needed for HOST_PROC setup — keep only that usage)

```go
package metrics

import (
	"context"
	"errors"
	"os"
	"runtime"

	"github.com/docker/docker/api/types/system"
	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/disk"
	"github.com/shirou/gopsutil/v4/mem"
)

type NodeMetricsDockerClient interface {
	Info(ctx context.Context) (system.Info, error)
}

type CollectNodeMetricsAction struct {
	dockerClient NodeMetricsDockerClient
	devMode      bool
}

func NewCollectNodeMetricsAction(dockerClient NodeMetricsDockerClient, devMode bool) *CollectNodeMetricsAction {
	if _, err := os.Stat("/host/proc"); err == nil {
		os.Setenv("HOST_PROC", "/host/proc")
	}
	return &CollectNodeMetricsAction{dockerClient: dockerClient, devMode: devMode}
}

func (a *CollectNodeMetricsAction) Execute(ctx context.Context) (NodeMetrics, error) {
	if a.devMode {
		return a.collectFromDocker(ctx)
	}
	return collectFromProc()
}
```

(Rest of file unchanged — `collectFromDocker` and `collectFromProc` stay the same.)

**Step 4: Run tests**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./metrics/... -v`
Expected: All PASS.

**Step 5: Verify full build**

Run: `go -C /Users/clovismuneza/Apps/kontainer/agent build ./...`
Expected: SUCCESS — all compilation errors resolved.

**Step 6: Commit**

```bash
git add agent/metrics/collect_node_metrics.go agent/metrics/collect_node_metrics_test.go
git commit -m "refactor: thread dev flag into CollectNodeMetricsAction instead of env var"
```

---

### Task 5: Remove ParseDevFollowers (now unused)

**Files:**
- Modify: `agent/cluster/dev_discovery.go`
- Modify: `agent/cluster/dev_discovery_test.go`

**Step 1: Remove `ParseDevFollowers` from dev_discovery.go**

Delete the `ParseDevFollowers` function (lines 32-46 of `dev_discovery.go`). It's no longer called — `cfg.Cluster.DevFollowers` is already a `[]string`.

**Step 2: Remove `TestParseDevFollowers` from dev_discovery_test.go**

Delete the test for the removed function.

**Step 3: Run tests**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./cluster/... -v`
Expected: All remaining tests PASS.

**Step 4: Verify full build**

Run: `go -C /Users/clovismuneza/Apps/kontainer/agent build ./...`
Expected: SUCCESS.

**Step 5: Run full test suite**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./...`
Expected: All PASS.

**Step 6: Commit**

```bash
git add agent/cluster/dev_discovery.go agent/cluster/dev_discovery_test.go
git commit -m "refactor: remove ParseDevFollowers, dev_followers is now a native YAML list"
```

---

### Task 6: Create example config and dev config files

**Files:**
- Create: `agent/kontainer.example.yml`
- Create: `agent/kontainer.yml` (for dev, gitignored)
- Delete: `agent/.env.example`
- Delete: `agent/.env` (if exists)
- Modify: `agent/.gitignore`

**Step 1: Create kontainer.example.yml**

```yaml
# Kontainer Agent Configuration
# Copy to kontainer.yml and adjust for your environment.
# Run: ./agent --config kontainer.yml

# Core
port: "8080"
agent_url: http://localhost:8080
role: leader                            # "leader" or "follower"

# Auth — used by leader only
jwt_secret: your-secret-here
# jwt_secret_file: /run/secrets/jwt_secret

github:
  client_id: ""
  # client_id_file: /run/secrets/github_client_id
  client_secret: ""
  # client_secret_file: /run/secrets/github_client_secret
  allowed_users: []                     # e.g. [alice, bob]
  allowed_orgs: []                      # e.g. [myorg]

# Cluster
cluster:
  internal_secret: ""
  # internal_secret_file: /run/secrets/internal_secret
  follower_service: kontainer-agent     # Swarm service name for follower discovery
  dev_followers: []                     # e.g. [localhost:8081, localhost:8082]

# Metrics — used by leader only
metrics:
  interval: 30s
  retention: 24h
  db_path: kontainer-metrics.db

# Development
dev: false                              # true = use Docker API for metrics instead of /proc
```

**Step 2: Create agent/kontainer.yml for dev**

Copy the content from the existing `.env` values:

```yaml
port: "8080"
agent_url: http://localhost:8080
role: leader
jwt_secret: your-secret-here
dev: true

github:
  client_id: ""
  client_secret: ""

cluster:
  internal_secret: dev-secret
  dev_followers: [localhost:8081, localhost:8082]

metrics:
  interval: 30s
  retention: 24h
  db_path: kontainer-metrics.db
```

**Step 3: Add kontainer.yml to .gitignore**

Add to `agent/.gitignore`:
```
kontainer.yml
```

**Step 4: Delete old env files**

```bash
rm agent/.env.example
rm -f agent/.env
```

**Step 5: Commit**

```bash
git add agent/kontainer.example.yml agent/.gitignore
git rm agent/.env.example
git rm -f agent/.env
git commit -m "feat: add kontainer.example.yml, remove .env files"
```

---

### Task 7: Update Dockerfile

**Files:**
- Modify: `agent/Dockerfile`

**Step 1: Update Dockerfile**

Remove the `ENV` block. Add a default config copy. Update entrypoint to use `--config`:

```dockerfile
# syntax=docker/dockerfile:1

# Build stage
FROM golang:1.25.1-alpine AS builder

RUN apk add --no-cache git ca-certificates tzdata

WORKDIR /build

COPY go.mod go.sum ./
RUN go mod download

COPY . .

RUN CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -ldflags="-s -w" -o agent .

# Runtime stage
FROM alpine:latest

RUN apk --no-cache add ca-certificates tzdata

WORKDIR /app

COPY --from=builder /build/agent /app/agent

# Create directories for data and config
RUN mkdir -p /data /etc/kontainer

# Copy example config as default
COPY --from=builder /build/kontainer.example.yml /etc/kontainer/kontainer.yml

EXPOSE 8080

RUN addgroup -g 1000 kontainer && \
    adduser -D -u 1000 -G kontainer kontainer && \
    chown -R kontainer:kontainer /app /data /etc/kontainer

USER kontainer

ENTRYPOINT ["/app/agent", "--config", "/etc/kontainer/kontainer.yml"]
```

**Step 2: Commit**

```bash
git add agent/Dockerfile
git commit -m "feat: update Dockerfile for YAML config (mount at /etc/kontainer/kontainer.yml)"
```

---

### Task 8: Update dev/Procfile

**Files:**
- Modify: `dev/Procfile`

**Step 1: Create dev config files for each role**

Create `dev/leader.yml`:
```yaml
port: "8080"
agent_url: http://localhost:8080
role: leader
jwt_secret: dev-jwt-secret
dev: true

cluster:
  internal_secret: dev-secret
  dev_followers: [localhost:8081, localhost:8082]

metrics:
  interval: 30s
  retention: 24h
  db_path: kontainer-metrics.db
```

Create `dev/follower.yml`:
```yaml
role: follower
dev: true

cluster:
  internal_secret: dev-secret
```

**Step 2: Update Procfile**

```
leader:    cd agent && DOCKER_HOST=tcp://localhost:2375 air -- --config ../dev/leader.yml
follower1: cd agent && DOCKER_HOST=tcp://localhost:2376 air -- --config ../dev/follower.yml
follower2: cd agent && DOCKER_HOST=tcp://localhost:2378 air -- --config ../dev/follower.yml
app:       pnpm --filter app tauri dev
```

Note: `DOCKER_HOST` remains as env var — it's a Docker client env var, not our config. The `--` tells air to pass the flag to the binary.

Follower config uses port default (8080), but needs different ports. Since we need different ports per follower, create `dev/follower1.yml` and `dev/follower2.yml` instead:

Create `dev/follower1.yml`:
```yaml
port: "8081"
role: follower
dev: true

cluster:
  internal_secret: dev-secret
```

Create `dev/follower2.yml`:
```yaml
port: "8082"
role: follower
dev: true

cluster:
  internal_secret: dev-secret
```

Updated Procfile:
```
leader:    cd agent && DOCKER_HOST=tcp://localhost:2375 air -- --config ../dev/leader.yml
follower1: cd agent && DOCKER_HOST=tcp://localhost:2376 air -- --config ../dev/follower1.yml
follower2: cd agent && DOCKER_HOST=tcp://localhost:2378 air -- --config ../dev/follower2.yml
app:       pnpm --filter app tauri dev
```

**Step 3: Commit**

```bash
git add dev/leader.yml dev/follower1.yml dev/follower2.yml dev/Procfile
git rm -f dev/follower.yml 2>/dev/null; true
git commit -m "feat: update dev/Procfile to use YAML config files"
```

---

### Task 9: Update README deployment examples

**Files:**
- Modify: `README.md` (lines ~138-189)

**Step 1: Update deployment section**

Replace the deployment and access control sections with YAML config examples using Docker configs/secrets:

````markdown
## Deployment

The leader agent reads configuration from a YAML file (default: `/etc/kontainer/kontainer.yml` inside the container).

**Leader service with Docker config:**

```yaml
services:
  kontainer-agent:
    image: prvious/kontainer-agent
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    configs:
      - source: kontainer-config
        target: /etc/kontainer/kontainer.yml
    secrets:
      - jwt_secret
      - github_client_secret
      - internal_secret

configs:
  kontainer-config:
    file: ./kontainer-leader.yml

secrets:
  jwt_secret:
    external: true
  github_client_secret:
    external: true
  internal_secret:
    external: true
```

Example `kontainer-leader.yml`:

```yaml
role: leader
agent_url: https://agent.example.com
jwt_secret_file: /run/secrets/jwt_secret
github:
  client_id: your-github-client-id
  client_secret_file: /run/secrets/github_client_secret
  allowed_users: [alice, bob]
cluster:
  internal_secret_file: /run/secrets/internal_secret
  follower_service: kontainer-follower
```

**Follower service (global, across worker nodes):**

```yaml
services:
  kontainer-follower:
    image: prvious/kontainer-agent
    deploy:
      mode: global
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    configs:
      - source: kontainer-follower-config
        target: /etc/kontainer/kontainer.yml
    secrets:
      - internal_secret

configs:
  kontainer-follower-config:
    file: ./kontainer-follower.yml

secrets:
  internal_secret:
    external: true
```

Example `kontainer-follower.yml`:

```yaml
role: follower
cluster:
  internal_secret_file: /run/secrets/internal_secret
```

## Access Control

Login can optionally be restricted by GitHub username and/or organization membership:

```yaml
github:
  allowed_users: [alice, bob]
  allowed_orgs: [my-org, my-other-org]
```

| `allowed_users` | `allowed_orgs` | Who can log in |
|---|---|---|
| unset | unset | All GitHub users (default) |
| set | unset | Only listed usernames |
| unset | set | Only members of listed orgs |
| set | set | Only listed usernames **who are also** members of listed orgs |

- `allowed_orgs` requires the `read:org` OAuth scope (added automatically when configured)

Non-matching users receive a `403 Forbidden` response during the OAuth callback.
````

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: update deployment examples to use YAML config with Docker secrets"
```

---

### Task 10: Update CLAUDE.md references

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Update config references**

Update references from env vars to YAML config:
- Change `KONTAINER_ROLE` references to `role` in YAML
- Change `KONTAINER_FOLLOWER_SERVICE` to `cluster.follower_service`
- Change `KONTAINER_DEV_FOLLOWERS` to `cluster.dev_followers`
- Change `KONTAINER_INTERNAL_SECRET` to `cluster.internal_secret`
- Update the discovery description to mention `--config` flag

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md config references from env vars to YAML"
```

---

### Task 11: Final verification

**Step 1: Run full Go test suite**

Run: `go test -C /Users/clovismuneza/Apps/kontainer/agent ./... -v`
Expected: All PASS.

**Step 2: Verify build**

Run: `go -C /Users/clovismuneza/Apps/kontainer/agent build -o /dev/null .`
Expected: SUCCESS.

**Step 3: Verify no env var references remain**

Run: `grep -r "os.Getenv\|godotenv\|KONTAINER_DEV\b\|KONTAINER_ROLE\b\|KONTAINER_INTERNAL_SECRET\b\|KONTAINER_METRICS\|KONTAINER_FOLLOWER" agent/ --include="*.go" | grep -v _test.go`

Expected: Only `DOCKER_HOST` or Docker-related env vars (which are Docker client config, not ours). Zero hits for our `KONTAINER_*` vars.

Note: `os.Setenv("HOST_PROC", ...)` in `collect_node_metrics.go` is fine — it's setting a gopsutil env var, not reading our config.
