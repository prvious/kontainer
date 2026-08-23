# Issue 58 mTLS evidence

Captured on 2026-08-23 from an ARM64 macOS host (`Darwin arm64`, Rust host
`aarch64-apple-darwin`). This evidence exercises local ephemeral listeners; it
does not claim Linux AMD64 or Linux ARM64 release verification.

## Fixture identities

The integration suite generates a new CA and leaf keys in a temporary directory
for every test run. The checked-in repository contains no private key material.
Both services load their leaf certificate, private key, and peer CA from
explicit file paths.

- Controller: `spiffe://kontainer.internal/swarm/cluster-test/controller/controller-a`
- Valid node agent: `spiffe://kontainer.internal/swarm/cluster-test/node/node-1/agent`
- Wrong workload: the controller identity above presented to the controller as
  a node agent
- Wrong cluster: `spiffe://kontainer.internal/swarm/cluster-other/node/node-1/agent`
- Unknown CA: the valid node-agent URI signed by an untrusted test CA
- Missing certificate: trusted server CA with no client identity

The services extract the URI SAN from the certificate verified by rustls at the
Tonic boundary. They reject absent identities, an unexpected trust domain or
URI shape, the wrong cluster, the wrong workload kind, and a protobuf
`agent_id` that differs from the authenticated certificate's node ID. Source IP
and certificate common name are not identity inputs.

## Exchange and rejection result

Command:

```text
cargo test --manifest-path server/Cargo.toml --test mtls_handshake -- --nocapture
```

The accepted exchange used protocol package `kontainer.agent.v1`, controller
port `49886`, and agent port `49887`. The rejection listener used port `49883`.
All ports were kernel-assigned loopback ports and will vary between runs.

```text
mTLS evidence: protocol=kontainer.agent.v1 controller=spiffe://kontainer.internal/swarm/cluster-test/controller/controller-a agent=spiffe://kontainer.internal/swarm/cluster-test/node/node-1/agent controller_port=49886 agent_port=49887 result=accepted
mTLS rejection evidence: port=49883 missing_cert=rejected unknown_ca=rejected plaintext=rejected wrong_cluster=cluster-other:rejected wrong_workload=controller-as-agent:rejected mismatched_claim=rejected
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

The suite also proves that cancellation interrupts an in-flight agent RPC,
joins its server task within two seconds, joins the controller HTTP and RPC
tasks under one cancellation token, and stops both listeners from accepting new
connections after shutdown.

## Dependency graph check

`cargo tree` reports one selected runtime/TLS stack in the workspace:
`rustls 0.23.43`, `hyper 1.11.0`, and `tokio 1.53.1`.

## Explicit limitations

The PKI is test-only and intentionally generated at runtime. Production CA
selection, enrollment, rotation, revocation, Swarm membership, delegation
grants, and multi-controller routing remain follow-up work.
