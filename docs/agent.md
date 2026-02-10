# Kontainer Agent Deployment Guide

This guide covers deploying the Kontainer agent on Docker Swarm clusters in various configurations.

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
- Access to `ghcr.io/prvious/kontainer-agent` image (publicly available)

## Single-Node Cluster

For development or single-node setups, deploy only the leader agent.

### Using docker run

```bash
docker run -d \
  --name kontainer-agent \
  -p 8080:8080 \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v kontainer-data:/data \
  -e KONTAINER_ROLE=leader \
  -e AGENT_URL=http://your-server:8080 \
  -e JWT_SECRET=your-jwt-secret-here \
  -e GITHUB_CLIENT_ID=your-github-client-id \
  -e GITHUB_CLIENT_SECRET=your-github-client-secret \
  ghcr.io/prvious/kontainer-agent:latest
```

### Using Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  kontainer-agent:
    image: ghcr.io/prvious/kontainer-agent:latest
    ports:
      - "8080:8080"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - kontainer-data:/data
    environment:
      KONTAINER_ROLE: leader
      AGENT_URL: http://your-server:8080
      JWT_SECRET: your-jwt-secret-here
      GITHUB_CLIENT_ID: your-github-client-id
      GITHUB_CLIENT_SECRET: your-github-client-secret
      KONTAINER_METRICS_DB: /data/kontainer-metrics.db
    restart: unless-stopped

volumes:
  kontainer-data:
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
docker network create --driver overlay --attachable kontainer-network
```

### Leader Service

Deploy the leader as a single-replica service on a manager node:

```bash
docker service create \
  --name kontainer-leader \
  --replicas 1 \
  --constraint 'node.role==manager' \
  --network kontainer-network \
  --publish published=8080,target=8080 \
  --mount type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock \
  --mount type=volume,source=kontainer-data,target=/data \
  -e KONTAINER_ROLE=leader \
  -e AGENT_URL=http://your-server:8080 \
  -e JWT_SECRET=your-jwt-secret-here \
  -e GITHUB_CLIENT_ID=your-github-client-id \
  -e GITHUB_CLIENT_SECRET=your-github-client-secret \
  -e KONTAINER_INTERNAL_SECRET=your-internal-secret-here \
  -e KONTAINER_FOLLOWER_SERVICE=kontainer-follower \
  -e KONTAINER_METRICS_DB=/data/kontainer-metrics.db \
  ghcr.io/prvious/kontainer-agent:latest
```

### Follower Service

Deploy followers as a global service across all nodes:

```bash
docker service create \
  --name kontainer-follower \
  --mode global \
  --network kontainer-network \
  --mount type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock \
  -e KONTAINER_ROLE=follower \
  -e KONTAINER_INTERNAL_SECRET=your-internal-secret-here \
  ghcr.io/prvious/kontainer-agent:latest
```

**Important Configuration Notes:**

- `KONTAINER_INTERNAL_SECRET`: Shared secret for leader ↔ follower authentication. Must be identical on leader and all followers.
- `KONTAINER_FOLLOWER_SERVICE`: Name of the follower service (default: `kontainer-agent`). The leader uses this to discover follower tasks via the Swarm API.
- Network: Both services must be on the same overlay network for internal communication.

## Leader Volume Configuration

The leader stores time-series node metrics in an embedded SQLite database. Followers are stateless and do not require persistent storage.

### Volume Requirements

- **Path**: `/data` (inside container)
- **Environment Variable**: `KONTAINER_METRICS_DB=/data/kontainer-metrics.db`
- **Recommended**: Use a named Docker volume or bind mount to a persistent directory

### Volume Examples

**Named Volume (Recommended):**

```bash
docker service create \
  --mount type=volume,source=kontainer-data,target=/data \
  ...
```

**Bind Mount:**

```bash
docker service create \
  --mount type=bind,source=/opt/kontainer/data,target=/data \
  ...
```

Ensure the directory exists on the host:

```bash
sudo mkdir -p /opt/kontainer/data
sudo chown -R 1000:1000 /opt/kontainer/data
```

**In Docker Compose:**

```yaml
volumes:
  - kontainer-data:/data  # Named volume
  # OR
  - /opt/kontainer/data:/data  # Bind mount
```

## Environment Variables Reference

### Leader Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `KONTAINER_ROLE` | Agent role | `leader` |
| `AGENT_URL` | Public URL of the agent | `http://your-server:8080` |
| `JWT_SECRET` | Secret for signing JWT tokens | `your-jwt-secret-here` |
| `GITHUB_CLIENT_ID` | GitHub OAuth app client ID | `Ov23li...` |
| `GITHUB_CLIENT_SECRET` | GitHub OAuth app client secret | `1234abcd...` |

### Follower Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `KONTAINER_ROLE` | Agent role | `follower` |
| `KONTAINER_INTERNAL_SECRET` | Shared secret for internal auth | `your-internal-secret-here` |

### Multi-Node Cluster Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `KONTAINER_INTERNAL_SECRET` | Shared secret for leader ↔ follower auth | (none) | `your-internal-secret-here` |
| `KONTAINER_FOLLOWER_SERVICE` | Follower service name for discovery | `kontainer-agent` | `kontainer-follower` |

### Optional Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | HTTP server port | `8080` |
| `KONTAINER_METRICS_INTERVAL` | Metrics collection interval | `30s` |
| `KONTAINER_METRICS_RETENTION` | Metrics retention period | `24h` |
| `KONTAINER_METRICS_DB` | Path to SQLite database | `kontainer-metrics.db` |
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
    image: ghcr.io/prvious/kontainer-agent:latest
    networks:
      - kontainer-network
    ports:
      - "8080:8080"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - kontainer-data:/data
    environment:
      KONTAINER_ROLE: leader
      AGENT_URL: http://your-server:8080
      JWT_SECRET: your-jwt-secret-here
      GITHUB_CLIENT_ID: your-github-client-id
      GITHUB_CLIENT_SECRET: your-github-client-secret
      KONTAINER_INTERNAL_SECRET: your-internal-secret-here
      KONTAINER_FOLLOWER_SERVICE: kontainer-follower
      KONTAINER_METRICS_DB: /data/kontainer-metrics.db
      KONTAINER_METRICS_INTERVAL: 30s
      KONTAINER_METRICS_RETENTION: 24h
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
    image: ghcr.io/prvious/kontainer-agent:latest
    networks:
      - kontainer-network
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    environment:
      KONTAINER_ROLE: follower
      KONTAINER_INTERNAL_SECRET: your-internal-secret-here
    deploy:
      mode: global
      restart_policy:
        condition: on-failure
        delay: 5s
        max_attempts: 3

networks:
  kontainer-network:
    driver: overlay
    attachable: true

volumes:
  kontainer-data:
```

### Deploy the Stack

Save the file as `kontainer-stack.yml` and deploy:

```bash
docker stack deploy -c kontainer-stack.yml kontainer
```

### Verify Deployment

Check service status:

```bash
docker stack services kontainer
docker service logs kontainer_leader
docker service logs kontainer_follower
```

View running tasks:

```bash
docker service ps kontainer_leader
docker service ps kontainer_follower
```

### Access the Agent

The leader API will be available at: `http://your-server:8080`

Test the health endpoint:

```bash
curl http://your-server:8080/health
```

## Troubleshooting

### Leader Cannot Discover Followers

- Ensure `KONTAINER_INTERNAL_SECRET` matches on leader and followers
- Verify both services are on the same overlay network
- Check `KONTAINER_FOLLOWER_SERVICE` matches the follower service name
- View leader logs: `docker service logs kontainer_leader`

### Follower Health Check Failing

- Ensure the follower service is running: `docker service ps kontainer_follower`
- Check follower logs: `docker service logs kontainer_follower`
- Verify Docker socket is mounted: `/var/run/docker.sock`

### Database Persistence Issues

- Verify the volume is mounted: `docker volume inspect kontainer-data`
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
5. **Internal Secret Rotation**: Regularly rotate `KONTAINER_INTERNAL_SECRET` and restart services

## Upgrading

To upgrade to a newer version:

```bash
# Update the image
docker service update --image ghcr.io/prvious/kontainer-agent:v0.2.0 kontainer_leader
docker service update --image ghcr.io/prvious/kontainer-agent:v0.2.0 kontainer_follower
```

Or redeploy the stack with the new tag in `kontainer-stack.yml`.
