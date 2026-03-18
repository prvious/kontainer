## Confirmed Features (Building These)

**Core Infrastructure**

- Multi-agent architecture (leader + follower) with Docker Swarm
- Agent REST API (not a Docker socket proxy)
- YAML config with `file:` and `${ENV}` resolution for secrets
- SQLite for agent storage (deploy history, secrets, settings)
- Tauri store plugin for app settings

**Authentication & Authorization**

- GitHub App via manifest flow (one-click setup)
- OAuth login through the GitHub App
- OIDC for GitHub Actions (zero-secret deploys)
- Deploy tokens for non-GitHub CI/CD
- Roles & permissions (admin / operator / viewer)
- Setup wizard with setup token on first boot

**Deployment Pipeline**

- Deploy through agent API (Option B)
- Stack file validation and preview before deploying
- Diff against currently running state
- Real-time rollout monitoring with task-level progress
- Auto-rollback on failure
- Deploy history with full timeline and audit trail
- Custom GitHub Action (`kontainer-dev/deploy-action`)
- GHCR image pulls via GitHub App installation tokens

**Secret Management (Kontainer Vault)**

- Path-based secret organization
- AES-256-GCM encryption with master key from Docker secret
- Secret versioning (last 10 versions)
- Soft deletes with undelete
- Label-based injection at deploy time (`kontainer.secrets.map`)
- Path-based bulk injection (`kontainer.secrets.path`)
- `${ENVIRONMENT}` interpolation in secret paths
- Missing secret policy (fail / warn / ignore)
- Pluggable provider interface (SQLite built-in, Vault and AWS later)
- Usage tracking (which services reference which secrets)

**Container Management**

- Service list, detail, scale, restart, rollback, env var editing, image update
- Container exec / shell via xterm.js WebSocket terminal
- User selection for exec (detect from `/etc/passwd`)
- Shell detection (try bash → sh → ash)
- Multiple concurrent terminal sessions
- Task management per service

**Stack Management**

- List, deploy, remove stacks
- CodeMirror editor for compose files, configs, env vars

**Logs**

- Per-service log streaming (WebSocket, 5000-line buffer)
- Unified logs page with multi-service streaming
- Stack-level log streaming (all services in a stack)
- Color coding per service
- Service filter, log level filter, node filter
- Throttling for high-volume services

**Node Management**

- List nodes, change availability, promote/demote
- Node exec / host shell via nsenter (admin only, opt-in)
- Node resource metrics (CPU, memory, disk)

**Networking, Volumes, Secrets, Configs**

- Full CRUD for Docker networks, volumes, secrets, configs

**Dashboard**

- Cluster health overview
- Stats cards, nodes breakdown, stacks summary

**Settings**

- App settings: refresh interval, theme, terminal defaults, log defaults, date format, editor settings, confirmation dialogs, startup behavior, notifications
- Agent settings: stored in YAML, managed via UI
- Connection profiles: per-cluster URL, display name, custom refresh interval

**GitHub Integration Page**

- GitHub App details and status
- Authentication settings (allowed org, default role)
- User management with role assignment
- OIDC deploy policies (repo → stack → branch → role)
- Deploy token management (create, revoke, expire)
- Registry (GHCR) status and auto-pull toggle
- Generate deploy workflow button
- Credential rotation and disconnect (danger zone)

**Dev Environment**

- 3-node DinD cluster (manager + 2 workers)
- Goreman with Air hot reload
- Seed services for testing
- Dev-mode resource metrics via `docker.Info()`

---

## Future Ideas (Might Build Someday)

**Advanced Deployments**

- Canary deployments (deploy new version at 1 replica, monitor via Traefik, auto-promote or rollback)
- Scheduled deployments (deploy at 2am when traffic is lowest)
- Service dependency ordering (deploy in order, wait for health checks)
- Database snapshots before deploy (auto-backup, restore on rollback)
- Config drift detection (running state vs compose file)
- Docker Compose → Swarm migration assistant
- Environment cloning (clone a stack as staging with env overrides)

**Autoscaler**

- Metrics-based autoscaling (CPU/memory thresholds)
- Configurable per service (min/max replicas, cooldown, scale up/down thresholds)
- Webhook notification when max replicas reached

**Kontainer Init Binary**

- Tiny Go binary added to Dockerfiles via `COPY --from`
- Fetches secrets from agent on container startup
- Works regardless of who deploys
- `syscall.Exec` replaces init process with the real app

**Traefik Integration**

- Live request feed from Traefik access logs
- Per-service metrics (requests/sec, p95 latency, error rates, status codes)
- Filtering by service, status, method

**Notifications & Webhooks**

- Slack integration (deploy completed, failed, service unhealthy, autoscaler events)
- Discord integration
- PagerDuty integration
- Generic webhooks with configurable event types
- Recent delivery history with status codes

**Observability**

- Incident timeline (unified view of events during an outage)
- Service health rules (custom rules like "restart if p95 > 500ms for 30s")
- Service benchmarking (quick load test via Traefik)
- Rollout progress indicator on service cards
- Cost estimation per service based on resource usage

**UI/UX**

- Splash screen
- Command palette (Cmd+K)
- Log search, filter, highlight, export
- Change diff before applying any mutation
- Pinned/favorited services
- Network dependency graph
- System notifications on failures
- Shareable dashboard links (read-only, time-limited)
- Container file browser

**CLI Tool**

- `kontainer` CLI in Go
- Same API as desktop app
- Auth via OIDC or deploy tokens
- Commands: services ls, scale, deploy, exec, logs

**Additional Integrations**

- GitLab App support
- Google OAuth provider
- AWS Secrets Manager provider
- HashiCorp Vault provider
- Private registry management

**Infrastructure (Way Later)**

- AWS cloud provider awareness (spin up VMs when autoscaler hits limits)
- Kubernetes support
- Kontainer operator for K8s
- Image vulnerability summary
- Audit log
- Service templates
- Scheduled scaling
- Mobile companion
