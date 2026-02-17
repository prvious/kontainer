# YAML-Only Agent Configuration

**Date:** 2026-02-17
**Status:** Approved

## Summary

Replace all environment variable configuration in the Go agent with a single YAML config file (`kontainer.yml`). Clean break — no env var fallback.

## Motivation

Better UX. A single YAML file is easier to read, version, and maintain than scattered env vars. Users can leverage Docker configs and secrets natively.

## YAML Schema

```yaml
# kontainer.yml

# Core
port: 8080
agent_url: http://localhost:8080
role: leader                            # "leader" or "follower"

# Auth (leader only)
jwt_secret: ""
jwt_secret_file: ""

github:
  client_id: ""
  client_id_file: ""
  client_secret: ""
  client_secret_file: ""
  allowed_users: []                     # e.g. [clovismuneza]
  allowed_orgs: []                      # e.g. [prvious]

# Cluster
cluster:
  internal_secret: ""
  internal_secret_file: ""
  follower_service: kontainer-agent
  dev_followers: []                     # e.g. [localhost:8081, localhost:8082]

# Metrics (leader only)
metrics:
  interval: 30s
  retention: 24h
  db_path: kontainer-metrics.db

# Development
dev: false
```

## Key Decisions

- **`_file` suffix convention:** Any secret field has a `_file` variant that reads from a file path (e.g. Docker secrets at `/run/secrets/`). File takes precedence if both are set.
- **Native YAML lists:** `allowed_users`, `allowed_orgs`, `dev_followers` are proper lists instead of comma-separated strings.
- **`dev` is a boolean** instead of a string check.
- **`docker_node_id` excluded:** Runtime concern, auto-detected from Docker daemon.

## Config Loading

### File Resolution

1. `--config /path/to/kontainer.yml` flag (if provided)
2. `./kontainer.yml` in current working directory
3. If neither found: fatal error with helpful message

### Secret Resolution

For each secret field pair (e.g. `jwt_secret` + `jwt_secret_file`):
- If `_file` is set: read file, trim whitespace, use as value
- If only direct value is set: use it
- If both set: file takes precedence
- If neither: empty string (validation may catch later)

### Validation (at startup, fatal on failure)

- `role` must be `"leader"` or `"follower"`
- Follower mode requires `cluster.internal_secret` (resolved)
- Leader mode warns if `jwt_secret`, `github.client_id`, or `github.client_secret` are empty
- `metrics.interval` and `metrics.retention` must parse as valid Go durations

## Go Struct

```go
type Config struct {
    Port     string       `yaml:"port"`
    AgentURL string       `yaml:"agent_url"`
    Role     string       `yaml:"role"`

    JWTSecret     string  `yaml:"jwt_secret"`
    JWTSecretFile string  `yaml:"jwt_secret_file"`

    GitHub  GitHubConfig  `yaml:"github"`
    Cluster ClusterConfig `yaml:"cluster"`
    Metrics MetricsConfig `yaml:"metrics"`
    Dev     bool          `yaml:"dev"`
}
```

## Dependencies

- **Add:** `gopkg.in/yaml.v3`, `flag` (stdlib)
- **Remove:** `github.com/joho/godotenv`

## Scope of Changes

### Modified
- `agent/config/config.go` — rewrite: YAML parsing, `_file` resolution, validation
- `agent/main.go` — remove godotenv, add `--config` flag, simplify parsing
- `agent/metrics/collect_node_metrics.go` — read `dev` from config struct
- `agent/go.mod` / `agent/go.sum` — add yaml.v3, remove godotenv
- `agent/Dockerfile` — remove ENV lines, document config mount
- `dev/Procfile` — use `--config` flag instead of inline env vars
- `README.md` — update deployment examples

### Added
- `agent/kontainer.example.yml` — documented example config
- `agent/kontainer.yml` — dev default (gitignored)

### Deleted
- `agent/.env.example`
- `agent/.env`

### Not touched
- Frontend code, Rust/Tauri layer, any Go code outside config loading path
