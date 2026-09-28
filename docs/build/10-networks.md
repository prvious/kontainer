# 10. Networks

## Goal

The network list across the cluster, with removal that explains why it is blocked, and a minimal
create dialog. This spec also introduces the controller's manager-scoped Docker engine that spec 11
extends, because overlay networks are created and removed on a manager.

Design: Networks `isFfK` (light `J0XJhi`); state `UPWcn` (03.S10 remove blocked popover and
built-in tooltip), caption `qynvY`. Create dialog is gap G1 (interim, modelled on `iPOa2`).

## Server

### Manager engine (new port)

- `server/src/swarm/engine.rs`: a `ManagerEngine` trait owned by the controller with the methods
  this spec and spec 11 need. The implementation lives in the engine crate
  (`connect_for_manager` exists) and is constructed from `settings.docker.host` (spec 02). If the
  host is not a Swarm manager (from `describe()`), the controller logs a warning at startup and
  manager-scoped routes return 503 `not_a_manager`.
- This spec's methods: `list_networks` (cluster-wide `network ls` on the manager, which includes
  every swarm-scope network), `inspect_network(id)` (endpoints: container names and ids),
  `create_network(name, driver, attachable, subnet, labels)`, `remove_network(id)`.

### Agent

- `ListNetworks`: id, name, driver, scope, subnets from IPAM, `containers` (endpoint count),
  created, labels, flags `attachable`, `ingress`, `internal`, `builtin` (name in `host`, `bridge`,
  `none`, `docker_gwbridge`, `ingress`).
- `InspectNetwork(id)` → attached container names; `RemoveNetwork(id)` for local networks.
- Containers (spec 07 DTO) gain `networks: [names]` so the Containers page can filter by network.
  Add the `network` filter there as part of this spec.

### Controller routes

- `GET /api/v1/networks` → merge of the manager's cluster-wide list (swarm scope) with the fan-out
  of local-scope networks per node; rows carry `scope`, `nodeIds` (local: one node; swarm: nodes
  where endpoints exist), `containers` summed, `builtin`.
- `GET /api/v1/networks/{id}?node=` → inspect with attached containers (manager for swarm scope,
  the node agent for local scope).
- `POST /api/v1/networks` `{ name, driver: "overlay" | "bridge", attachable, nodeId?, subnet? }`
  → 202 `network.create`; overlay runs on the manager, bridge requires `nodeId` and runs on that
  agent. Validation: name unique across the current list, subnet a valid CIDR when given.
- `DELETE /api/v1/networks/{id}?node=` → 202 `network.remove`; built-ins → 400 `builtin_network`;
  endpoints present → the operation fails with `in_use` and the attached names.

## Desktop

- Route `/networks`. Tauri: `list_networks`, `inspect_network`, `create_network`,
  `remove_network`. Hook `useNetworks()`; `useNetworkDetail(id)` for the popover, fetched on open.
- Header: `NETWORKS`, subtitle `N networks · a overlay · b bridge · c host`, REFRESH, CREATE
  NETWORK (primary).
- Network Overview strip (`NRFey`): legend `N ACTIVE` (with endpoints) / `N EMPTY`; readout: a
  Ring `ACTIVE n / total`; the `TRAFFIC · 60m` trend renders as a skeleton (ADR 0010).
- TableToolbar: search over name and id; Filters: driver, scope, node; Sort: name.
- DataTable: NAME, ID (12 chars), DRIVER, SCOPE, SUBNET (`—` when none), CONTAINERS, CREATED,
  actions: trash. Built-in networks keep a disabled trash with a Tooltip Rich "Built-in networks
  can't be removed" (03.S10).
- Remove (03.S10): clicking the trash on a network with endpoints opens an anchored Popover: "Can't
  remove app-network · 6 containers are attached", the container names, and SHOW CONTAINERS which
  navigates to `/containers?network=<name>`. With no endpoints, a plain Cancel / Remove
  ConfirmDialog. Docker errors are never shown raw; the operation's `in_use` message is.
- Create (G1): dialog with NAME (validated on blur for uniqueness and the name pattern), DRIVER
  radio overlay (default) / bridge, NODE select shown only for bridge, ATTACHABLE switch for
  overlay, optional SUBNET validated as CIDR. CREATE NETWORK disabled while invalid, pending during
  the operation, closes on success with the row highlighted for 3 s.

## Tests

- Controller: merge of swarm-scope and local-scope lists with a fake manager engine and fake
  agents; built-in rejection; validation table.
- Real cluster: create an attachable overlay, it lists with scope swarm; create a bridge on
  worker-2; removing `ingress` is rejected; removing the seed stack's network fails with `in_use`
  and the attached names.
- Desktop: popover with names and navigation; disabled built-ins; dialog validation; overlay vs
  bridge fields.

## Done when

- The dev cluster's networks list with correct scope and counts, the blocked-removal popover shows
  the seed containers, and creating and removing a network works end to end; all checks pass;
  `PROGRESS.md` updated; ADR for the container DTO `networks` field.
