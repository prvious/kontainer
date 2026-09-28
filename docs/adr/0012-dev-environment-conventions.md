# 0012. Development environment conventions

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/02-dev-environment.md

## Context

Spec 02 wires the DinD Swarm, dev certificates, a controller, and three agents together. Several
details were left open: the TLS server names peers verify, how the long agent environment is kept
readable, and whether the controller may depend on the engine crate before spec 10.

## Decision

- **Docker host type.** `kontainer-engine` owns `DockerHost` (`unix:///path`, a bare absolute path,
  or `tcp://host:port`; every other scheme is rejected). `LocalEngineOptions.socket_path` became
  `host`. The real-engine tests read the same `KONTAINER_TEST_DOCKER_SOCKET` variable through this
  parser, so a `tcp://` value works there too.
- **Environment gate.** The agent reads `KONTAINER_ENV` (`production` default, `development`,
  `test`, anything else fails startup) and refuses a `tcp://` `KONTAINER_DOCKER_HOST` in production.
  The controller reads `settings.docker.host` and refuses `tcp://` unless the Loco environment is
  development or test, checked in `after_context` before any listener binds.
- **Server names.** Every dev leaf has exactly one SPIFFE URI SAN plus a DNS SAN that peers use as
  the TLS server name: `<node-id>.agent.kontainer.internal` for agents and
  `controller.kontainer.internal` for the controller, with `localhost` and `127.0.0.1` added for
  local dialing. Spec 05 derives per-node server names from this convention.
- **One CA.** `dev/pki/agent-ca.pem` signs the controller and the agents and is the trust root on
  both sides.
- **Launchers.** `dev/agent.sh <node>` holds the agent environment (paths, ports, node id) so the
  Procfile stays one line per process; `dev/agents.sh start|stop|status` runs the three in the
  background with logs and pids in `dev/logs/`. Agents run with `cargo … -p kontainer-node-agent`
  because the binary lives in a workspace member, not the root package.
- **Controller dependency.** `kontainer-engine` moves from the controller's dev-dependencies to its
  dependencies now (it parses `settings.docker.host`); spec 10 builds the manager engine from it.
  No external crate is added.
- `pnpm dev:controller` sets `KONTAINER_DOCKER_HOST=tcp://localhost:2375`, matching the Procfile,
  because the dev controller's manager engine targets the DinD manager.

## Consequences

- A fresh checkout needs `pnpm dev:pki` before `pnpm dev:controller`; the controller fails fast on
  missing certificate files because agent RPC is enabled in development.
- Production certificate issuance (spec G17, deferred) must follow the same SAN convention.
