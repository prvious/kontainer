# Docker Engine qualification

This slice qualifies the local Unix-socket connection, API negotiation,
`/_ping`, `/version`, `/info`, and paged node-local container inventory. It
does not establish the final Kontainer 1.0 Engine support range or qualify
any Swarm mutation.

The `Engine qualification` workflow runs the same adapter test against these
pinned real daemon images:

| Daemon image | Negotiated API | Purpose |
|---|---:|---|
| `docker:27.5.1-dind` | `1.47` | Provisional lower evidence point |
| `docker:29.7.2-dind` | `1.53` | Current-line compatibility evidence point |

Each lane records the image digest, daemon version and API range, negotiated
API, runner architecture, Rust version, command, and raw test result in an
uploaded evidence artifact. It also creates one running and one exited
fixture container, walks the inventory with page size one, stops the daemon,
and proves that the adapter reports a typed `ConnectionUnavailable` outcome.

If both lanes pass, Engine API 1.47, represented by Docker Engine 27.5.1, is
the provisional evidence floor for this walking skeleton. This is not the
advertised Kontainer 1.0 support floor: that claim remains blocked on the full
resource and Swarm compatibility suite.

Run one qualification locally against an already isolated daemon socket with:

```shell
KONTAINER_TEST_DOCKER_SOCKET=/path/to/docker.sock \
KONTAINER_TEST_ENGINE_IMAGE=docker:27.5.1-dind \
KONTAINER_TEST_ENGINE_VERSION=27.5.1 \
KONTAINER_TEST_ENGINE_API_VERSION=1.47 \
cargo test --manifest-path server/Cargo.toml \
  --package kontainer-engine --features real-engine-tests \
  --test real_docker qualifies_configured_real_engine -- --nocapture
```

Create the inventory fixtures first with an isolated daemon client:

```shell
docker --host unix:///path/to/docker.sock pull alpine:3.20
docker --host unix:///path/to/docker.sock run -d --name kontainer-inventory-running --network none alpine:3.20 sleep infinity
docker --host unix:///path/to/docker.sock run --name kontainer-inventory-exited --network none alpine:3.20 true
cargo test --manifest-path server/Cargo.toml \
  --package kontainer-engine --features real-engine-tests \
  --test real_docker qualifies_paged_container_inventory -- --nocapture
```

## Container inventory

- Default page size is 50; the hard maximum is 100. Larger requests are
  rejected before any Engine access.
- Pages follow the daemon's newest-created order. The cursor is an opaque
  `v1:<full-container-id>` token of at most 67 bytes; callers pass it back
  unchanged to read older containers.
- The adapter requests `all=true`, `size=false`, and `limit=page_size+1`,
  adding only a `before=<cursor>` filter for later pages. No other filter,
  text search, or sorting parameter is sent.
- A live collection may change between calls. Pagination is best-effort:
  created or removed containers can shift pages, and a cursor whose target
  was removed is rejected as invalid pagination. Restart traversal from the
  first page in that case.
- Each summary carries only the full container ID, canonical display name,
  image reference, typed state, bounded status text (256 bytes), and
  creation time. Environment, command, labels, mounts, ports, networking,
  and raw inspect data never cross the port.

Do not point this test at an untrusted remote endpoint. Remote Docker is outside
the approved initial product scope.
