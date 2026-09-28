#!/usr/bin/env bash
# Development mTLS material for the controller and the three dev node agents.
#
# One ECDSA P-256 CA signs every leaf. Each leaf carries exactly one SPIFFE
# URI SAN (the workload identity rpc-auth checks), a DNS SAN the peer uses as
# its TLS server name, and localhost/127.0.0.1 for local dialing. Existing
# files are kept unless --force is given.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKI_DIR="$SCRIPT_DIR/pki"
CLUSTER_ID="development"
DAYS=365
FORCE=0

for argument in "$@"; do
  case "$argument" in
    --force) FORCE=1 ;;
    *) echo "usage: $0 [--force]" >&2; exit 64 ;;
  esac
done

command -v openssl >/dev/null || { echo "openssl is required" >&2; exit 1; }
mkdir -p "$PKI_DIR"
chmod 700 "$PKI_DIR"

if [[ "$FORCE" == 1 ]]; then
  rm -f "$PKI_DIR"/*.pem "$PKI_DIR"/*.srl
fi

new_key() {
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$1" 2>/dev/null
  chmod 600 "$1"
}

if [[ ! -f "$PKI_DIR/agent-ca.pem" || ! -f "$PKI_DIR/agent-ca-key.pem" ]]; then
  echo "==> Creating development CA"
  new_key "$PKI_DIR/agent-ca-key.pem"
  openssl req -x509 -new -sha256 -days "$DAYS" \
    -key "$PKI_DIR/agent-ca-key.pem" \
    -subj "/CN=Kontainer development CA" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign,digitalSignature" \
    -addext "subjectKeyIdentifier=hash" \
    -out "$PKI_DIR/agent-ca.pem"
  # A new CA invalidates every leaf signed by the old one.
  rm -f "$PKI_DIR"/controller*.pem "$PKI_DIR"/node-*.pem
fi

# issue <file-stem> <common-name> <dns-name> <spiffe-uri>
issue() {
  local stem="$1" common_name="$2" dns_name="$3" uri="$4"
  local certificate="$PKI_DIR/$stem.pem" key="$PKI_DIR/$stem-key.pem"
  if [[ -f "$certificate" && -f "$key" ]]; then
    echo "    $stem exists, keeping it"
    return
  fi
  echo "==> Issuing $stem ($uri)"
  new_key "$key"
  local extensions
  extensions="$(mktemp)"
  cat >"$extensions" <<EOF
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=serverAuth,clientAuth
subjectAltName=URI:$uri,DNS:$dns_name,DNS:localhost,IP:127.0.0.1
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid
EOF
  openssl req -new -key "$key" -subj "/CN=$common_name" 2>/dev/null |
    openssl x509 -req -sha256 -days "$DAYS" \
      -CA "$PKI_DIR/agent-ca.pem" -CAkey "$PKI_DIR/agent-ca-key.pem" \
      -set_serial "0x$(openssl rand -hex 16)" \
      -extfile "$extensions" \
      -out "$certificate" 2>/dev/null
  rm -f "$extensions"
}

issue controller "controller" "controller.kontainer.internal" \
  "spiffe://kontainer.internal/swarm/$CLUSTER_ID/controller/dev"
for node in manager worker-1 worker-2; do
  issue "node-$node" "$node" "$node.agent.kontainer.internal" \
    "spiffe://kontainer.internal/swarm/$CLUSTER_ID/node/$node/agent"
done

for certificate in controller node-manager node-worker-1 node-worker-2; do
  openssl verify -CAfile "$PKI_DIR/agent-ca.pem" "$PKI_DIR/$certificate.pem" >/dev/null
done
echo "==> Development PKI ready in $PKI_DIR"
