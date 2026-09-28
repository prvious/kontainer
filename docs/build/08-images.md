# 08. Images

## Goal

The cross-node image list with removal (typed confirmation when in use) and pulling with per-node
progress.

Design: Images `OJDW6` (light `y36cL7`); states `iuoVz` (03.S7 delete confirmation) and `D9mu7`
(03.S8 pull image). Captions `YA90y`, `Q8es5`. Foundations Confirm Dialog `KIuxe`. Gaps: G12
(registry credentials, deferred).

## Server

### Agent

- `ListImages`: id, repo tags, repo digests, size bytes, created, `used_by` (count of containers
  on this node whose image id matches), `dangling`.
- `RemoveImage(id, force)`; `PullImage(reference)` as a server-streaming RPC yielding
  `{ layer_id, status, current, total }` progress messages and a final result. Engine trait gains
  `list_images`, `remove_image`, `pull_image` (returns a stream of typed progress, no Bollard
  types). Registry authentication is out of scope; pulls are anonymous.

### Controller

- `GET /api/v1/images` → fan-out; images with the same id on several nodes merge into one row
  with `nodeIds`, `usedBy` summed, size once. `nodes` outcomes and `snapshotAt` as in spec 05.
- `DELETE /api/v1/images/{id}` body `{ nodeIds: [...] | "all", force }` → 202 `image.remove`;
  the executor removes on each listed node with concurrency 4 and reports per-node results.
- `POST /api/v1/images/pulls` `{ reference, nodeIds }` → 202 `image.pull`; the executor runs
  every node in parallel, consumes each progress stream, and reports detail
  `{ nodes: [{ nodeId, status: pulling | done | failed, layersDone, layersTotal, bytes, error? }] }`.
  One failing node never stops the others. Registry errors are passed through verbatim in `error`.
- Reference validation at the edge: `[host[:port]/]name[:tag|@digest]` with the Docker reference
  grammar; invalid → 400 with the field error.

## Desktop

- Route `/images`. Tauri: `list_images`, `remove_image(id, nodeIds, force, idempotencyKey)`,
  `pull_image(reference, nodeIds, idempotencyKey)`.
- Hook `useImages()` → `['images', profileId]`, polling from settings, `keepPreviousData`.

### Page

- Header: `IMAGES`, subtitle `N images · X GB total`, REFRESH, PULL IMAGE (primary).
- Image Storage strip (`InNiu`): legend `N IN USE` / `N UNUSED`; readout `DISK USAGE` as a number
  with unit and no ring (there is no real ceiling); the `PULLS · 14d` trend has no data source and
  renders as a skeleton (ADR 0010).
- TableToolbar: search over tag and id; Sort: created desc (default), size, name.
- DataTable: NAME / TAG (first tag; `<none>` for dangling), IMAGE ID (12-char short id, full on
  hover), SIZE, CREATED (relative), USED BY, NODES (small badges), actions: trash.
- Delete (03.S7): dialog shows impact first ("In use by N containers: names…" gathered from the
  containers query), then typed confirmation of the full `name:tag` when `usedBy > 0`; DELETE IMAGE
  stays at 40 % until the text matches; unused images get plain Cancel / Delete. Default targets:
  every node that has it. Success fades the row and toasts "Deletes are final".
- Pull (03.S8): dialog with reference field validated on blur, node checkboxes defaulting to all
  online nodes, PULL. Per-node progress bars from the operation detail; a failing node shows the
  registry error verbatim with the hint "Private registries need credentials, not built yet" and a
  per-node RETRY that starts a new pull for that node. RUN IN BACKGROUND closes the dialog and a
  loading toast tracks the operation until terminal; the new image row appears on the next poll.

## Tests

- Agent: list with used-by counts; remove in-use image without force fails; pull progress stream
  with the fake engine.
- Controller: merge by id across nodes; reference validation table (valid and invalid cases);
  pull executor keeps going after one node fails; remove per-node results.
- Real cluster: pull `alpine:3.20` on all three nodes; remove it from one node; remove an in-use
  image without force fails with a readable error.
- Desktop: typed confirmation only when in use; pull dialog validation and per-node progress;
  background toast.

## Done when

- Pulling an image from the desktop shows three progress bars finishing, the image appears in the
  list on every node, and deleting it removes it; all checks pass; `PROGRESS.md` updated.
