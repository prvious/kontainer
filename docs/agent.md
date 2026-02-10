# Devoper Agent Deployment Guide

This guide covers deploying the Devoper agent on Docker Swarm clusters in various configurations.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Single-Node Cluster](#single-node-cluster)
- [Multi-Node Cluster](#multi-node-cluster)
- [Leader Volume Configuration](#leader-volume-configuration)
- [Environment Variables Reference](#environment-variables-reference)
- [Full Stack File Example](#full-stack-file-example)

## Prerequisites

- Docker Swarm initialized (`docker swarm init`)
- GitHub OAuth application credentials (for authentication)
- Access to `ghcr.io/prvious/devoper-agent` image (publicly available)

## Single-Node Cluster

For development or single-node setups, deploy only the leader agent.

### Using docker run

```bash
docker run -d \
  --name devoper-agent \
  -p 8080:8080 \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v devoper-data:/data \
  -e DEVOPER_ROLE=leader \
  -e AGENT_URL=http://your-server:8080 \
  -e JWT_SECRET=your-jwt-secret-here \
  -e GITHUB_CLIENT_ID=your-github-client-id \
  -e GITHUB_CLIENT_SECRET=your-github-client-secret \
  ghcr.io/prvious/devoper-agent:latest
```

### Using Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  devoper-agent:
    image: ghcr.io/prvious/devoper-agent:latest
    ports:
      - "8080:8080"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - devoper-data:/data
    environment:
      DEVOPER_ROLE: leader
      AGENT_URL: http://your-server:8080
      JWT_SECRET: your-jwt-secret-here
      GITHUB_CLIENT_ID: your-github-client-id
      GITHUB_CLIENT_SECRET: your-github-client-secret
      DEVOPER_METRICS_DB: /data/devoper-metrics.db
    restart: unless-stopped

volumes:
  devoper-data:
```

Deploy with:

```bash
docker-compose up -d
```

## Multi-Node Cluster

In a multi-node Swarm cluster, deploy one leader (on a manager node) and followers (global service on all worker nodes).

### Architecture

- **Leader**: Exposes the full API, handles authentication, discovers and communicates with followers
- **Followers**: Lightweight agents that run on each worker node, exposing internal metrics and container operations

### Network Setup

Create an overlay network for leader-follower communication:

```bash
docker network create --driver overlay --attachable devoper-network
```

### Leader Service

Deploy the leader as a single-replica service on a manager node:

```bash
docker service create \
  --name devoper-leader \
  --replicas 1 \
  --constraint 'node.role==manager' \
  --network devoper-network \
  --publish published=8080,target=8080 \
  --mount type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock \
  --mount type=volume,source=devoper-data,target=/data \
  -e DEVOPER_ROLE=leader \
  -e AGENT_URL=http://your-server:8080 \
  -e JWT_SECRET=your-jwt-secret-here \
  -e GITHUB_CLIENT_ID=your-github-client-id \
  -e GITHUB_CLIENT_SECRET=your-github-client-secret \
  -e DEVOPER_INTERNAL_SECRET=your-internal-secret-here \
  -e DEVOPER_FOLLOWER_SERVICE=devoper-follower \
  -e DEVOPER_METRICS_DB=/data/devoper-metrics.db \
  ghcr.io/prvious/devoper-agent:latest
```

### Follower Service

Deploy followers as a global service across all nodes:

```bash
docker service create \
  --name devoper-follower \
  --mode global \
  --network devoper-network \
  --mount type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock \
  -e DEVOPER_ROLE=follower \
  -e DEVOPER_INTERNAL_SECRET=your-internal-secret-here \
  ghcr.io/prvious/devoper-agent:latest
```

**Important Configuration Notes:**

- `DEVOPER_INTERNAL_SECRET`: Shared secret for leader ↔ follower authentication. Must be identical on leader and all followers.
- `DEVOPER_FOLLOWER_SERVICE`: Name of the follower service (default: `devoper-agent`). The leader uses this to discover follower tasks via the Swarm API.
- Network: Both services must be on the same overlay network for internal communication.

## Leader Volume Configuration

The leader stores time-series node metrics in an embedded SQLite database. Followers are stateless and do not require persistent storage.

### Volume Requirements

- **Path**: `/data` (inside container)
- **Environment Variable**: `DEVOPER_METRICS_DB=/data/devoper-metrics.db`
- **Recommended**: Use a named Docker volume or bind mount to a persistent directory

### Volume Examples

**Named Volume (Recommended):**

```bash
docker service create \
  --mount type=volume,source=devoper-data,target=/data \
  ...
```

**Bind Mount:**

```bash
docker service create \
  --mount type=bind,source=/opt/devoper/data,target=/data \
  ...
```

Ensure the directory exists on the host:

```bash
sudo mkdir -p /opt/devoper/data
sudo chown -R 1000:1000 /opt/devoper/data
```

**In Docker Compose:**

```yaml
volumes:
  - devoper-data:/data  # Named volume
  # OR
  - /opt/devoper/data:/data  # Bind mount
```

## Environment Variables Reference

### Leader Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `DEVOPER_ROLE` | Agent role | `leader` |
| `AGENT_URL` | Public URL of the agent | `http://your-server:8080` |
| `JWT_SECRET` | Secret for signing JWT tokens | `your-jwt-secret-here` |
| `GITHUB_CLIENT_ID` | GitHub OAuth app client ID | `Ov23li...` |
| `GITHUB_CLIENT_SECRET` | GitHub OAuth app client secret | `1234abcd...` |

### Follower Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `DEVOPER_ROLE` | Agent role | `follower` |
| `DEVOPER_INTERNAL_SECRET` | Shared secret for internal auth | `your-internal-secret-here` |

### Multi-Node Cluster Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `DEVOPER_INTERNAL_SECRET` | Shared secret for leader ↔ follower auth | (none) | `your-internal-secret-here` |
| `DEVOPER_FOLLOWER_SERVICE` | Follower service name for discovery | `devoper-agent` | `devoper-follower` |

### Optional Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | HTTP server port | `8080` |
| `DEVOPER_METRICS_INTERVAL` | Metrics collection interval | `30s` |
| `DEVOPER_METRICS_RETENTION` | Metrics retention period | `24h` |
| `DEVOPER_METRICS_DB` | Path to SQLite database | `devoper-metrics.db` |
| `GITHUB_ALLOWED_ORGS` | Comma-separated GitHub org allowlist | (none) |

### GitHub Organization Allowlist

Restrict access to members of specific GitHub organizations:

```bash
-e GITHUB_ALLOWED_ORGS=your-org,another-org
```

Only users who are members of the specified organizations will be able to authenticate.

## Full Stack File Example

Complete Docker Stack file for a multi-node Swarm cluster with leader and followers:

```yaml
version: '3.8'

services:
  leader:
    image: ghcr.io/prvious/devoper-agent:latest
    networks:
      - devoper-network
    ports:
      - "8080:8080"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - devoper-data:/data
    environment:
      DEVOPER_ROLE: leader
      AGENT_URL: http://your-server:8080
      JWT_SECRET: your-jwt-secret-here
      GITHUB_CLIENT_ID: your-github-client-id
      GITHUB_CLIENT_SECRET: your-github-client-secret
      DEVOPER_INTERNAL_SECRET: your-internal-secret-here
      DEVOPER_FOLLOWER_SERVICE: devoper-follower
      DEVOPER_METRICS_DB: /data/devoper-metrics.db
      DEVOPER_METRICS_INTERVAL: 30s
      DEVOPER_METRICS_RETENTION: 24h
      # Optional: Restrict to GitHub org members
      # GITHUB_ALLOWED_ORGS: your-org
    deploy:
      replicas: 1
      placement:
        constraints:
          - node.role == manager
      restart_policy:
        condition: on-failure
        delay: 5s
        max_attempts: 3

  follower:
    image: ghcr.io/prvious/devoper-agent:latest
    networks:
      - devoper-network
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    environment:
      DEVOPER_ROLE: follower
      DEVOPER_INTERNAL_SECRET: your-internal-secret-here
    deploy:
      mode: global
      restart_policy:
        condition: on-failure
        delay: 5s
        max_attempts: 3

networks:
  devoper-network:
    driver: overlay
    attachable: true

volumes:
  devoper-data:
```

### Deploy the Stack

Save the file as `devoper-stack.yml` and deploy:

```bash
docker stack deploy -c devoper-stack.yml devoper
```

### Verify Deployment

Check service status:

```bash
docker stack services devoper
docker service logs devoper_leader
docker service logs devoper_follower
```

View running tasks:

```bash
docker service ps devoper_leader
docker service ps devoper_follower
```

### Access the Agent

The leader API will be available at: `http://your-server:8080`

Test the health endpoint:

```bash
curl http://your-server:8080/health
```

## Troubleshooting

### Leader Cannot Discover Followers

- Ensure `DEVOPER_INTERNAL_SECRET` matches on leader and followers
- Verify both services are on the same overlay network
- Check `DEVOPER_FOLLOWER_SERVICE` matches the follower service name
- View leader logs: `docker service logs devoper_leader`

### Follower Health Check Failing

- Ensure the follower service is running: `docker service ps devoper_follower`
- Check follower logs: `docker service logs devoper_follower`
- Verify Docker socket is mounted: `/var/run/docker.sock`

### Database Persistence Issues

- Verify the volume is mounted: `docker volume inspect devoper-data`
- Check directory permissions (UID 1000 owns `/data`)
- View leader logs for SQLite errors

### GitHub OAuth Not Working

- Verify `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET` are correct
- Ensure `AGENT_URL` matches your GitHub OAuth callback URL
- Check callback URL in GitHub app settings: `http://your-server:8080/auth/github/callback`

## Security Considerations

1. **Secrets Management**: Use Docker secrets or external secret management for sensitive values
2. **Network Isolation**: Use internal overlay networks for follower communication
3. **TLS/HTTPS**: Place a reverse proxy (nginx, Traefik) in front of the leader for HTTPS
4. **Organization Allowlist**: Use `GITHUB_ALLOWED_ORGS` to restrict access
5. **Internal Secret Rotation**: Regularly rotate `DEVOPER_INTERNAL_SECRET` and restart services

## Upgrading

To upgrade to a newer version:

```bash
# Update the image
docker service update --image ghcr.io/prvious/devoper-agent:v0.2.0 devoper_leader
docker service update --image ghcr.io/prvious/devoper-agent:v0.2.0 devoper_follower
```

Or redeploy the stack with the new tag in `devoper-stack.yml`.
