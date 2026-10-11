# 19. Self-setup on a Swarm

## Goal

Kontainer deploys the way Portainer does: the operator writes one stack file, behind whatever
reverse proxy they run, and nothing else. No certificates to generate, no node ids or addresses to
fill in. This repository builds and publishes the images; deploying them is the operator's job.
Decisions: ADR 0045, ADR 0046. Amends ADR 0010 (node enrollment) and ADR 0042.

## Images

- `server/Dockerfile` builds both binaries once, with targets `controller` and `agent`, on
  `rust:1.98-slim-trixie`, and runs them on `gcr.io/distroless/cc-debian13` as root (they need the
  Docker socket). The controller image carries `config/production.yaml` and `LOCO_ENV=production`.
- `.github/workflows/images.yml` builds `linux/amd64` and `linux/arm64` on native runners and
  publishes `ghcr.io/prvious/kontainer/controller` and `ghcr.io/prvious/kontainer/agent`: `main`
  moves `latest`, a `v1.2.3` tag publishes `1.2.3`, both keep the commit SHA. Pull requests that
  touch the image only build it.

## Controller

- Serves plain HTTP on 5150 by default; the operator's proxy terminates TLS. Setting
  `PUBLIC_HTTP_TLS_CERTIFICATE_PATH` and `PUBLIC_HTTP_TLS_PRIVATE_KEY_PATH` switches to built-in TLS.
- Migrates its database on start (`DB_AUTO_MIGRATE`, default `true`). `HOST` defaults to
  `http://localhost:5150`; only GitHub login reads it. `CLUSTER_ID` defaults to `default`.
- Without `AGENT_RPC_TLS_*` paths, issues its agent RPC certificate from `KONTAINER_SECRET` (or
  `KONTAINER_SECRET_FILE`) and refuses to start when neither the paths nor the secret is set.

## Agent

- Without `KONTAINER_NODE_RPC_TLS_*` paths, issues its certificate from the same secret.
- `KONTAINER_NODE_ID` defaults to the node's Swarm id (its hostname outside a Swarm), so Docker must
  answer at startup when it is unset. `KONTAINER_CLUSTER_ID` defaults to `default`,
  `KONTAINER_CONTROLLER_RPC_ENDPOINT` to `https://controller:50051`, and
  `KONTAINER_CONTROLLER_SERVER_NAME` to `controller.kontainer.internal`.
- `KONTAINER_NODE_ADVERTISE_ADDRESS` defaults to a specific binding, else to this host's address on
  the route to the controller, with the binding's port.

## Unchanged

The development cluster, `dev/pki.sh`, and every mTLS check. Only who issues the certificates moves.

## Tests

- rpc-auth: the same secret derives the same CA and another does not; issued files carry the
  workload identity, load as a matching pair, and are owner-only; short secrets fail without echoing.
- Agent: node id from the Swarm id, then the hostname, else a clear error; advertise address on the
  route to the controller; unresolved node id and address validate.
- Controller: optional RPC bundle; files win, else the secret, else a startup error; the rendered
  production config is plaintext unless given a certificate; plaintext serves on every interface.
- mTLS: secret-issued controller and agent complete a verified Describe; another secret is refused.

## Done when

- Both images build; a local stack of PostgreSQL, controller and agent with only `DATABASE_URL`
  and `KONTAINER_SECRET` set reaches `/health/ready` and the agent checks in; all checks pass.
