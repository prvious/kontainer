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
jwt_secret: file:/run/secrets/jwt_secret

github:
  client_id: ${GITHUB_CLIENT_ID}
  client_secret: file:/run/secrets/github_client_secret
  allowed_users: []                     # e.g. [clovismuneza]
  allowed_orgs: []                      # e.g. [prvious]

# Cluster
cluster:
  internal_secret: file:/run/secrets/internal_secret
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

## Value Resolution

All string config values pass through a single `resolveValue` function that handles three resolution methods:

| Syntax | Source | Use case |
|---|---|---|
| `file:/run/secrets/x` | Docker secret (or any file) | Production |
| `${ENV_VAR}` | Environment variable | Development |
| `plain-value` | Literal string | Non-sensitive values |

```go
func resolveValue(val string) (string, error) {
    // file:/path/to/secret → read the file, trim whitespace
    if strings.HasPrefix(val, "file:") {
        data, err := os.ReadFile(strings.TrimPrefix(val, "file:"))
        if err != nil {
            return "", fmt.Errorf("reading secret from %q: %w", val, err)
        }
        return strings.TrimSpace(string(data)), nil
    }

    // ${ENV_VAR} → expand env vars (for dev; unexpanded vars become empty string)
    return os.ExpandEnv(val), nil
}
```

`os.ExpandEnv` handles the `${ENV_VAR}` expansion natively and passes through literal strings unchanged.

## Example Configs

### Production (Docker secrets)

```yaml
# kontainer.yml
auth:
  jwt_secret: file:/run/secrets/jwt-secret

github:
  client_id: your-github-app-client-id   # non-sensitive, literal is fine
  client_secret: file:/run/secrets/github-client-secret
  allowed_org: "my-org"

cluster:
  internal_secret: file:/run/secrets/internal-secret

server:
  port: 8080
```

### Development (env vars)

```yaml
# kontainer.dev.yml
auth:
  jwt_secret: ${JWT_SECRET}

github:
  client_id: ${GITHUB_CLIENT_ID}
  client_secret: ${GITHUB_CLIENT_SECRET}
  allowed_org: "my-org"

server:
  port: 8080
```

## Key Decisions

- **`file:` prefix:** Any string value prefixed with `file:` is treated as a file path to read. This replaces the old `_file` suffix convention. Errors if the file does not exist.
- **`${ENV_VAR}` expansion:** Any string value is passed through `os.ExpandEnv`, enabling `${VAR}` syntax for dev convenience. Unexpanded variables resolve to empty string.
- **Plain literals:** Strings without either prefix are used as-is. Suitable for non-sensitive values.
- **Simpler struct:** No more paired fields (e.g. `jwt_secret` + `jwt_secret_file`). Each field is just a `string` that gets resolved via `resolveValue`.
- **Native YAML lists:** `allowed_users`, `allowed_orgs`, `dev_followers` are proper lists instead of comma-separated strings.
- **`dev` is a boolean** instead of a string check.
- **`docker_node_id` excluded:** Runtime concern, auto-detected from Docker daemon.

## Config Loading

### File Resolution

1. `--config /path/to/kontainer.yml` flag (if provided)
2. `./kontainer.yml` in current working directory
3. If neither found: fatal error with helpful message

### Secret Resolution

After YAML parsing, `resolveSecrets` calls `resolveValue` on all sensitive string fields:
- `jwt_secret`
- `github.client_id`, `github.client_secret`
- `cluster.internal_secret`

Resolution order for `file:` prefix: read file → trim whitespace → use as value. Errors fatally on missing file.

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

    JWTSecret string       `yaml:"jwt_secret"`

    GitHub  GitHubConfig  `yaml:"github"`
    Cluster ClusterConfig `yaml:"cluster"`
    Metrics MetricsConfig `yaml:"metrics"`
    Dev     bool          `yaml:"dev"`
}

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
```

## Dependencies

- **Add:** `gopkg.in/yaml.v3`, `flag` (stdlib)
- **Remove:** `github.com/joho/godotenv`

## Scope of Changes

### Modified
- `agent/config/config.go` — rewrite: YAML parsing, `resolveValue` for `file:`/`${ENV_VAR}`, validation
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
