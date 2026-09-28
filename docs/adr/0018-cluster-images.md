# 0018. Cluster images, removal, and pulls

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/08-images.md

## Context

Spec 08 adds the cross-node image list, image removal, and anonymous pulls with per-node progress.
That adds protobuf RPCs, public HTTP routes, native-client calls, and Tauri IPC commands, and the
design (`OJDW6`, `iuoVz`, `D9mu7`) shows a few details the spec leaves out.

## Decision

- **Protobuf (additive).** `NodeAgent` gains `ListImages`, `RemoveImage(image_id, force)` (an in-use
  image without force is `FAILED_PRECONDITION` `image_in_use`), and the server-streaming
  `PullImage(reference)` that yields `{ layer_id, status, current, total }` and ends with the pull
  or one `error` event. Registry messages are passed on verbatim, bounded by the agent.
- **Engine.** `list_images`, `remove_image`, and `pull_image` sit behind `ContainerEngine`. A pull
  is a stream of Kontainer-owned `PullEvent`s; `used_by` counts the node's containers by image id.
- **Public API.** `GET /api/v1/images` fans out and merges by image id (`nodeIds`, `usedBy` summed,
  size once). `DELETE /api/v1/images/{imageId}` takes `{ nodeIds: [...] | "all", force }`;
  `POST /api/v1/images/pulls` takes `{ reference, nodeIds }`. Both answer 202 with an operation.
  Validation failures are 400 `{ "error": "invalid_request", "fields": { field: problem } }`. The
  reference follows the Docker grammar (`[host[:port]/]name[:tag|@digest]`, 512 characters at most,
  name 255, tag 128) and gains `:latest` when it names neither a tag nor a digest, because a bare
  name pulls every tag.
- **Executors.** `image.remove` removes with concurrency 4; with `"all"`, a member without the image
  is recorded as skipped. `image.pull` runs every node in parallel and writes
  `{ nodes: [{ nodeId, status, layersDone, layersTotal, bytes, totalBytes, error? }] }` at most every
  500 ms and when a node finishes. One failing node never stops the others; the operation ends
  `partial_failure` (some failed) or `pull_failed` (all failed). A finished node reports every layer
  done, since Docker's containerd image store lists manifests and configs that never report "Pull
  complete". `totalBytes` is an addition to the spec's detail so bars can show bytes.
- **Native client and IPC.** `list_images`, `remove_image(image_id, ImageNodes, force, key)`, and
  `pull_image(reference, node_ids, key)`; `NativeClient::admit_with` admits under methods other than
  `POST`. The controller's reference check stays authoritative: its 400 maps to `invalid_request`.
- **Desktop.** `/images`, with `['images', profileId]` polled at the refresh interval.
  - DISK USAGE is the sum of each image's size times the number of nodes holding it (what the
    cluster stores); the header's "X GB total" counts each image once.
  - Tags from the containerd store (`name:tag@sha256:…`) display and confirm as `name:tag`.
  - The delete dialog's impact matches containers by tag, pinned digest, or id.
  - Settled operation toasts are keyed by the operation id, so RUN IN BACKGROUND's loading toast is
    replaced in place.
  - The pull dialog shows the design's CANCEL PULL, but it works only while the pull is still queued
    (ADR 0016); once any node is downloading, the button explains why it can't stop.
  - A per-node RETRY starts a new one-node pull, and the dialog shows each node's latest attempt.
- **Left out on purpose.** The design's row inspect (eye) action, the type/age filter chips, and the
  per-node durations ("pulled · 14s") have no spec or data source and are not built. The registry
  hint says "Private registries need credentials, not built yet" (G12) instead of pointing at a
  Settings → Registries page that doesn't exist.

## Consequences

- Layer counts during a pull come from the daemon's progress ids, which include non-layer blobs on
  the containerd store, so "layer 1/3" can read low until the node finishes.
- A pull cannot be cancelled once it starts; closing the dialog never stops it.
