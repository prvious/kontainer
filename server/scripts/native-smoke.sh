#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 <target-triple> <expected-uname-machine> <evidence-directory>" >&2
  exit 64
fi

target_triple="$1"
expected_machine="$2"
evidence_directory="$3"

case "$target_triple:$expected_machine" in
  x86_64-unknown-linux-gnu:x86_64) binary_machine="x86-64" ;;
  aarch64-unknown-linux-gnu:aarch64) binary_machine="ARM aarch64" ;;
  *)
    echo "unsupported native smoke target/machine: $target_triple/$expected_machine" >&2
    exit 64
    ;;
esac

actual_machine="$(uname -m)"
if [[ "$(uname -s)" != Linux || "$actual_machine" != "$expected_machine" ]]; then
  echo "native runner mismatch: expected $expected_machine, got $actual_machine" >&2
  exit 1
fi

mkdir -p "$evidence_directory/artifacts"
uname -a > "$evidence_directory/uname.txt"
rustc -Vv > "$evidence_directory/rustc.txt"
cargo -V > "$evidence_directory/cargo.txt"
git rev-parse HEAD > "$evidence_directory/commit.txt"
sha256sum server/Cargo.lock > "$evidence_directory/lockfile-sha256.txt"

cargo build \
  --manifest-path server/Cargo.toml \
  --locked \
  --release \
  --target "$target_triple" \
  --package kontainer-controller \
  --bin kontainer-controller

cargo build \
  --manifest-path server/Cargo.toml \
  --locked \
  --release \
  --target "$target_triple" \
  --package kontainer-node-agent \
  --bin kontainer-node-agent

cargo test \
  --manifest-path server/Cargo.toml \
  --locked \
  --release \
  --target "$target_triple" \
  --package kontainer-agent-proto \
  -- --nocapture 2>&1 | tee "$evidence_directory/protobuf-runtime-test.txt"

target_directory="server/target/$target_triple/release"
controller_binary="$target_directory/kontainer-controller"
node_agent_binary="$target_directory/kontainer-node-agent"

for binary in "$controller_binary" "$node_agent_binary"; do
  binary_format="$(file "$binary")"
  echo "$binary_format"
  if [[ "$binary_format" != *"ELF 64-bit"* || "$binary_format" != *"$binary_machine"* ]]; then
    echo "unexpected binary format: $binary_format" >&2
    exit 1
  fi
done | tee "$evidence_directory/binary-format.txt"

timeout --kill-after=2s 10s "$controller_binary" --help > "$evidence_directory/controller-help.txt"
timeout --kill-after=2s 10s "$node_agent_binary" --version > "$evidence_directory/node-agent-version.txt"

missing_config_exit=0
# With no certificate paths and no cluster secret the agent has no identity and must fail closed.
timeout --kill-after=2s 10s env -u KONTAINER_SECRET -u KONTAINER_SECRET_FILE \
  -u KONTAINER_NODE_RPC_TLS_CERTIFICATE_PATH -u KONTAINER_NODE_RPC_TLS_PRIVATE_KEY_PATH \
  -u KONTAINER_NODE_RPC_TLS_CLIENT_CA_PATH "$node_agent_binary" \
  > "$evidence_directory/node-agent-missing-config.txt" 2>&1 || missing_config_exit=$?
if [[ "$missing_config_exit" -ne 1 ]]; then
  echo "expected node-agent configuration exit 1, got $missing_config_exit" >&2
  exit 1
fi
grep -q "set KONTAINER_SECRET (or KONTAINER_SECRET_FILE)" \
  "$evidence_directory/node-agent-missing-config.txt"

install -m 0755 "$controller_binary" "$evidence_directory/artifacts/kontainer-controller"
install -m 0755 "$node_agent_binary" "$evidence_directory/artifacts/kontainer-node-agent"

sha256sum \
  "$evidence_directory/artifacts/kontainer-controller" \
  "$evidence_directory/artifacts/kontainer-node-agent" \
  > "$evidence_directory/artifact-sha256.txt"

{
  echo "target_triple=$target_triple"
  echo "runner_machine=$actual_machine"
  echo "commit=$(git rev-parse HEAD)"
  echo "lockfile_sha256=$(sha256sum server/Cargo.lock | cut -d ' ' -f 1)"
  echo "controller_sha256=$(sha256sum "$controller_binary" | cut -d ' ' -f 1)"
  echo "node_agent_sha256=$(sha256sum "$node_agent_binary" | cut -d ' ' -f 1)"
} > "$evidence_directory/manifest.txt"

cat "$evidence_directory/manifest.txt"
