# Missing Features & Placeholder Data

Items displayed in the UI with fake/placeholder data or NOOP buttons. Each needs backend work before it can be wired up.

---

## Dashboard (Cluster Overview)

- **Traefik Banner** — Shows static "Traefik Proxy · Connected · 1,247 req/s · 42ms p95 · 0.8% err". No traefik integration exists in the agent yet. Needs a traefik service discovery + metrics endpoint.

- **DEPLOY button** — NOOP. No deployment action exists in the agent API. Will need a deploy endpoint that triggers stack/service creation.

- **Network I/O metric card** — Shows placeholder data. The agent's `/api/metrics` endpoint does not return network throughput. Needs network I/O collection added to the metrics collector.

- **Container ports column** — Shows "—" for all rows. Neither `TaskItem` nor `ContainerMetrics` includes port mapping data. Needs the agent to return published ports per container.

- **Container metrics server-side filtering** — The `/api/metrics` endpoint returns ALL container metrics. The dashboard joins client-side with displayed tasks. A query param (e.g. `?containerIds=...`) would reduce payload size for large clusters.

## Status Bar

- **Git branch segment** — Shows static "main". No mechanism to detect or report the branch/version of the agent.

- **Docker version segment** — Shows static "Docker --". The agent's `/ping` response doesn't include the Docker engine version. Needs a version field added to the ping response.

- **WebSocket status indicator** — Shows static "WS --". Not wired to actual WebSocket connection state. Needs a connection health hook.

- **Log streams count** — Shows static "-- STREAMS". Not connected to any log stream count. Needs tracking of active log stream subscriptions.

## Sidebar

- **Agent version/uptime card** — Shows static "AGENT -- / uptime: --". No agent health endpoint returns version or uptime. Needs a `/api/health` or `/ping` extension with version and uptime fields.
