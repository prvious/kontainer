# 0020. Networks, the manager engine, and container networks

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/10-networks.md

## Context

Spec 10 lists networks across the cluster, removes them with an explanation when that's blocked,
and creates them from a minimal dialog. Overlay networks live on a Swarm manager, so this spec
also gives the controller its own manager-scoped Docker engine, which spec 11 extends. It also
asks for container `networks` so the Containers page can filter by network.

## Decision

- **Manager engine.** `server/src/swarm/engine.rs` defines the controller-owned `ManagerEngine`
  port: `list_networks`, `inspect_network`, `create_network`, `remove_network`. Its Docker
  implementation wraps the engine crate's `connect_for_manager` engine, built from
  `settings.docker.host`.
  - At `serve`, before the listeners accept requests, the controller calls `describe()` (5 s
    deadline) and keeps the engine only if Swarm manager operations are supported. Otherwise it
    logs one warning, and manager-scoped routes answer 503 `not_a_manager`: overlay create and
    swarm-scope removal.
  - The check runs once per start: a manager that was down at start stays absent until restart.
  - Executors get the engine through `ExecutionContext::with_manager`, which the worker carries
    beside the agent directory.
- **Engine.** `ContainerEngine` gains the four network methods, used by agents for local networks
  and by the manager adapter for swarm ones. Endpoint counts come from the running containers'
  network settings, because `network ls` doesn't carry them. Built-ins (`host`, `bridge`, `none`,
  `docker_gwbridge`, `ingress`) are refused before Docker is asked. Subnets must be canonical CIDRs
  (prefix 8 or more, no host bits), and names follow `[a-zA-Z0-9][a-zA-Z0-9_.-]*` up to 128.
- **Protobuf (additive).** `NodeAgent` gains `ListNetworks`, `InspectNetwork`, `CreateNetwork`
  (local bridge only), and `RemoveNetwork`. As with volumes (ADR 0019), a network with endpoints
  comes back as `in_use_by` names; built-ins are `INVALID_ARGUMENT` `builtin_network`.
- **Container DTO `networks`.** `ContainerSummary` gains `repeated string networks = 17` (at most
  16 names, sorted). It carries through the controller's `ContainerResponse.networks`, the native
  client's `ContainerItem.networks`, and the Tauri and TypeScript `ContainerView.networks`.
  Containers gain a `network` URL filter with a chip and a Filters section; the blocked-remove
  popover links to `/containers?network=<name>`.
- **Merge.** The swarm-scope networks come from the manager when it answered, since a worker only
  lists an overlay while a task uses it. Each node's endpoint count is summed, and `nodeIds` names
  the nodes with endpoints. Without the manager, swarm rows come from the nodes, once per id.
  Local networks are one row per node, so the manager's own locals come through its agent, not
  twice. The list carries `manager: ok | not_a_manager | unavailable`, and the page warns when it
  isn't `ok`.
- **Inspect.** `GET /api/v1/networks/{id}` without `node` asks every reachable node for its
  endpoints and names each container with its node. The spec says "manager for swarm scope", but a
  manager's inspect only shows its own endpoints. With `node`, only that agent is asked.
- **Admission.** A create checks name uniqueness against the merged list (Docker allows duplicate
  names), refuses built-in names (`reserved`), and needs `nodeId` for bridge and refuses it for
  overlay. A remove resolves the row from the list first. That gives 404 for an unknown id on that
  scope or node, 400 `builtin_network` for a built-in, and 503 `not_a_manager` for a swarm network
  without the manager.
- **Remove executor.** Swarm scope asks the nodes for endpoints and fails `in_use` with the names
  before the manager is touched. Local scope relies on the agent's refusal. Docker's wording never
  reaches the user.
- **Desktop.**
  - An empty network is removed through a plain Cancel / Remove confirmation. That's reversible
    (recreate it), unlike a volume's data.
  - The blocked-remove popover follows the design's copy ("VIEW CONTAINERS").
  - The create dialog uses the `onSettled` and `highlightedRow` pattern from ADR 0019.
  - The strip's Ring shows active over total networks, not the design's "subnet use", which has
    no data source. TRAFFIC · 60m is a skeleton (ADR 0010).
  - The design's inspect and connect row actions are not built; no spec asks for them.

## Consequences

- `ingress` shows zero containers, because its endpoints are Swarm load-balancer sandboxes, not
  containers.
- The list does one manager call plus one call per node, so a slow manager delays it up to the
  per-node deadline.
