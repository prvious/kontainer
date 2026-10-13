# 0045. Derive cluster trust from one shared secret

- Status: accepted
- Date: 2026-10-10
- Spec: docs/build/19-self-setup.md
- Amends: ADR 0010 (node enrollment), ADR 0042

## Context

Private RPC is mandatory mTLS with workload identities in each certificate. Until now an operator
issued a CA and one certificate per node by hand, as `dev/pki.sh` does for the dev cluster. The owner
wants deployment to be a stack file and nothing else, the way Portainer's agent works. Enrollment
(an agent asks the controller to sign its key) would need a new RPC, a listener that accepts clients
without certificates, a CA stored in PostgreSQL, and still a shared secret to authenticate the ask.

## Decision

- Every controller and agent holding `KONTAINER_SECRET` (or `KONTAINER_SECRET_FILE`) derives the
  same cluster CA: an Ed25519 key seeded with SHA-256 of a domain label and the secret, self-signed
  with fixed fields. Each process then signs a fresh P-256 certificate for its own identity
  (`controller.kontainer.internal`, `{node_id}.agent.kontainer.internal`) at startup.
- It writes the certificate, key and CA as owner-only files and hands their paths to the existing
  `MutualTlsIdentityFiles`. Every verifier, identity rule and handshake stays as it was.
- Configured certificate paths still win, so the dev cluster and operator-issued PKI keep working.
- The secret must be at least 64 hex characters, which is what `openssl rand -hex 32` prints.
  Requiring hex, not just length, rules out passphrases and repeated words.
- Leaf certificates keep rcgen's default validity and nothing renews them.

## Consequences

- No enrollment protocol, no new table, no new port; agents stay stateless.
- Trust is the secret. Anyone holding it can mint any controller or agent identity, which is also
  true of an enrollment scheme authenticated by the same secret. Revoking means rotating the secret
  and restarting the stack.
- Every certificate the CA signs names its key and carries its signature, and anything that can
  open a connection to an RPC port receives one (the controller's also listens on the proxy network
  it joins). A guessable secret could therefore be found offline with one SHA-256 per guess. The hex
  floor demands 32 random bytes instead; a slow KDF would protect chosen passphrases too, but needs a
  new dependency (Argon2 only reaches the controller through loco-rs) and is deferred.
- `rcgen` becomes a normal dependency of `kontainer-rpc-auth` (it was already in the lockfile), and
  `sha2` joins it from the workspace.
