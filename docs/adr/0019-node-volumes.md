# 0019. Node-scoped volumes, create, and remove

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/09-volumes.md

## Context

Spec 09 adds the cross-node volume list with create and remove. That adds protobuf RPCs, public
HTTP routes, native-client calls, and Tauri IPC commands, and runs into three Docker behaviours the
spec does not spell out.

## Decision

- **Protobuf (additive).** `NodeAgent` gains `ListVolumes`, `CreateVolume(name, driver, labels,
  driver_opts)`, and `RemoveVolume(name)`. An existing name is `ALREADY_EXISTS` `volume_exists`. A
  mounted volume is not an error: `RemoveVolumeResponse.in_use_by` names the containers and nothing
  is removed. A refusal is a normal answer, and the names are structured data rather than text in
  a status message.
- **Engine.** `list_volumes`, `create_volume`, and `remove_volume` sit behind `ContainerEngine`.
  Docker answers a create for an existing name with that volume, so the engine inspects first and
  refuses. `docker system df` now runs verbose, which keeps the category totals and adds each
  volume's size. The agent keeps those sizes in a `VolumeSizeCache` that the sampler refreshes on
  every twelfth sample; `ListVolumes` reads the cache and never walks volume contents.
- **Names.** Docker's rule is `[a-zA-Z0-9][a-zA-Z0-9_.-]+`, which needs at least two characters.
  The spec's `{0,254}` would allow one character that Docker then rejects, so the controller,
  agent, native client, and desktop all use `{1,254}` (2–255 characters).
- **Public API.** `GET /api/v1/volumes[?node=&name=]` returns one row per node and name, with
  outcomes and `snapshotAt`; an unknown `node` is 404. `POST /api/v1/volumes` takes `{ nodeId,
  name, driver, labels }` and accepts `driver: "local"` only. `DELETE
  /api/v1/volumes/{nodeId}/{name}` fails its operation with `volume_in_use` and the names in both
  the message and `{ inUseBy }`. Field errors reuse one `FieldErrorResponse` schema (renamed from
  spec 08's `ImageFieldErrorResponse`). The OpenAPI query allowlist gains the two list filters.
- **Desktop.**
  - Tracked operations can carry an `onSettled` callback, which the provider calls once after its
    toast. The create dialog uses it to close on success and to show the controller's message on
    failure.
  - The new row is highlighted through a `highlightedRow` search param. Any other navigation drops
    it, and one `setTimeout` in the success handler clears it after 3 s.
  - The name's conflict check is a query keyed by node and the name last left, so switching the
    node re-checks without an effect.
  - Removing an unattached volume asks the user to type its name, not the spec's plain
    Cancel / Remove. The standing security floor requires typed confirmation for destructive
    actions, and a removed volume's data cannot be recovered. Attached volumes keep the spec's
    rule: the trash is disabled and a Tooltip Rich lists the containers.
- **Dev cluster.** The seed stacks gain named volumes: `toolbox_redis-data` for redis, and
  `monitoring_collector-cache` for the global collector, so there is one per node. The page
  therefore has attached volumes to show.

## Consequences

- A new volume's size reads `—` ("sizes pending") until the node's next disk sample, up to a
  minute later.
- Volume sizes are only as fresh as the last verbose `df`, which also means one extra walk of the
  volume directories every twelfth sample on every node.
