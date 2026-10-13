# 0046. Serve plaintext behind the operator's reverse proxy

- Status: accepted
- Date: 2026-10-10
- Spec: docs/build/19-self-setup.md
- Amends: ADR 0014, the reset doc's public TLS rule (issue #77)

## Context

Production refused plaintext public HTTP and accepted only a certificate file, which the controller
cannot obtain itself (no ACME). Operators run Kontainer on clusters that already have Traefik,
HAProxy or similar terminating TLS with public certificates, as Portainer does on port 9000. The
desktop app trusts only public roots and refuses plain `http://` for anything but loopback, so it
still reaches the controller over HTTPS: through the proxy.

## Decision

- Plaintext is allowed in every environment and on any bind address. The loopback-bind and
  loopback-host checks are removed.
- `config/production.yaml` serves plaintext unless `PUBLIC_HTTP_TLS_CERTIFICATE_PATH` is set, in
  which case it serves the configured certificate exactly as before and still fails closed on a bad
  identity.

## Consequences

- The hop from the proxy to the controller is plaintext on the cluster's network. That network must
  be one the operator trusts; Swarm overlays can be created with `encrypted: true` when it isn't.
- A misconfigured deployment can expose the API over plain HTTP. The desktop app will not connect to
  it remotely, but other clients could, so the README tells operators to publish the controller only
  through their proxy.
- Tests that pinned the old refusal now pin the new behavior.
