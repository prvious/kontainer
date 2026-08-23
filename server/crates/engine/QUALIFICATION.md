# Docker Engine qualification

This slice qualifies only the local Unix-socket connection, API negotiation,
`/_ping`, `/version`, and `/info`. It does not establish the final Kontainer
1.0 Engine support range or qualify any Swarm mutation.

The `Engine qualification` workflow runs the same adapter test against these
pinned real daemon images:

| Daemon image | Purpose |
|---|---|
| `docker:27.5.1-dind` | Provisional lower evidence point |
| `docker:29.7.2-dind` | Current-line compatibility evidence point |

Each lane records the image digest, daemon version and API range, negotiated
API, runner architecture, Rust version, command, and raw test result in an
uploaded evidence artifact. It also stops the daemon and proves that the
adapter reports a typed `ConnectionUnavailable` outcome.

If both lanes pass, Docker Engine 27.5.1 is the provisional evidence floor for
this walking skeleton. This is not the advertised Kontainer 1.0 support floor:
that claim remains blocked on the full resource and Swarm compatibility suite.

Run one qualification locally against an already isolated daemon socket with:

```shell
KONTAINER_TEST_DOCKER_SOCKET=/path/to/docker.sock \
KONTAINER_TEST_ENGINE_IMAGE=docker:27.5.1-dind \
KONTAINER_TEST_ENGINE_VERSION=27.5.1 \
cargo test --manifest-path server/Cargo.toml \
  --package kontainer-engine --features real-engine-tests \
  --test real_docker qualifies_configured_real_engine -- --nocapture
```

Do not point this test at an untrusted remote endpoint. Remote Docker is outside
the approved initial product scope.
