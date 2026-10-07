# 09. Volumes

## Goal

The cross-node volume list with create and remove. The Volume Explorer and File Preview screens
are deferred: their row action renders disabled.

Design: Volumes `jigga` (light `OZAJL`); state `iPOa2` (03.S9 create volume validation), caption
`KNWE2`. Deferred: Volume Explorer `5mQoG`, File Preview `mhztj`, states `D5EBq`, `NZNrn`
(gaps G18). Remove confirmation is gap G8.

## Server

### Agent

- `ListVolumes`: name, driver, mountpoint, created, labels, `used_by` (containers on this node
  mounting it), `size_bytes` (nullable, from the `docker system df` cache that spec 05 refreshes
  every twelfth sample).
- `CreateVolume(name, driver, labels, driver_opts)`, `RemoveVolume(name)`. Engine trait gains the
  three methods. Docker refuses to remove a volume that a container uses, with or without force;
  surface that as `in_use` with the container names.

### Controller

- `GET /api/v1/volumes` → fan-out; volumes are node-scoped so rows carry `nodeId` and the same
  name on two nodes is two rows. Query `?node=&name=` supports the dialog's conflict check.
- `POST /api/v1/volumes` `{ nodeId, name, driver, labels }` → 202 `volume.create`. Validation:
  name matches `[a-zA-Z0-9][a-zA-Z0-9_.-]{0,254}`, driver `local` only in this phase.
- `DELETE /api/v1/volumes/{nodeId}/{name}` → 202 `volume.remove`; `in_use` fails the operation
  with the names in the message.

## Desktop

- Route `/volumes`. Tauri: `list_volumes`, `create_volume`, `remove_volume`. Hook `useVolumes()`
  → `['volumes', profileId]`.
- Header: `VOLUMES`, subtitle `N volumes · X GB allocated` (sum of known sizes; append
  `· sizes pending` while any size is null), REFRESH, CREATE VOLUME (primary).
- Volume Storage strip (`htkOl`): legend `N ATTACHED` / `N UNATTACHED`; readout `ALLOCATED` as a
  number without a ring; `GROWTH · 30d` renders as a skeleton (no data source, ADR 0010).
- TableToolbar: search over name and mount point; Filters: node, driver, attached; Sort: size desc
  (default), name, created.
- DataTable: NAME, NODE, DRIVER, MOUNT POINT (truncated middle, full path in tooltip, copy on
  hover), SIZE (`—` when unknown), USED BY, CREATED, actions: Browse files (disabled, tooltip
  "Not built yet"), Remove.
- Create (03.S9): dialog with NODE select (online nodes), NAME validated on blur against that
  node through the conflict query (destructive helper "A volume named X already exists on
  worker-1. Pick another name or another node."), DRIVER fixed to `local`, optional LABELS rows.
  CREATE VOLUME disabled while any field is invalid; pending while the operation runs; on success
  the dialog closes and the new row is highlighted for 3 s (a `highlightedRow` search param that
  the page clears on the next navigation, no timers in effects beyond one `setTimeout` inside the
  event handler that opened it).
- Remove (G8, adjusted to Docker's rules): volumes with `usedBy > 0` have the trash icon disabled
  with a Tooltip Rich listing the containers; unattached volumes open a plain Cancel / Remove
  ConfirmDialog. Update `design-gaps.md` G8 to say so.

## Tests

- Agent: create, list with sizes from the cache, remove in-use → `in_use` with names.
- Controller: validation table for names; conflict query; executor results.
- Real cluster: create on worker-1, appears in the list with driver local, remove it.
- Desktop: dialog validation on blur, disabled create while invalid, highlight after success,
  disabled trash for attached volumes.

## Done when

- Creating and removing a volume on a chosen node works from the desktop with Operation Status and
  toasts; the seed stacks' volumes list as attached with their sizes; all checks pass;
  `PROGRESS.md` updated.
