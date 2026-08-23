#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 <target-triple> <expected-uname-machine> <evidence-directory>" >&2
  exit 64
fi

target_triple="$1"
expected_machine="$2"
evidence_directory="$3"

actual_machine="$(uname -m)"
if [[ "$actual_machine" != "$expected_machine" ]]; then
  echo "native runner mismatch: expected $expected_machine, got $actual_machine" >&2
  exit 1
fi

mkdir -p "$evidence_directory/artifacts"

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
  --target "$target_triple" \
  --package kontainer-agent-proto \
  -- --nocapture | tee "$evidence_directory/protobuf-runtime-test.txt"

target_directory="server/target/$target_triple/release"
controller_binary="$target_directory/kontainer-controller"
node_agent_binary="$target_directory/kontainer-node-agent"

file "$controller_binary" "$node_agent_binary" | tee "$evidence_directory/binary-format.txt"

case "$target_triple" in
  x86_64-unknown-linux-gnu)
    grep -q "x86-64" "$evidence_directory/binary-format.txt"
    ;;
  aarch64-unknown-linux-gnu)
    grep -q "ARM aarch64" "$evidence_directory/binary-format.txt"
    ;;
  *)
    echo "unsupported native smoke target: $target_triple" >&2
    exit 64
    ;;
esac

"$controller_binary" --help > "$evidence_directory/controller-help.txt"
"$node_agent_binary" --version > "$evidence_directory/node-agent-version.txt"

if env -u KONTAINER_NODE_ID "$node_agent_binary" \
  > "$evidence_directory/node-agent-missing-config.txt" 2>&1; then
  echo "node agent unexpectedly started without required configuration" >&2
  exit 1
fi
grep -q "required environment variable KONTAINER_NODE_ID is not set" \
  "$evidence_directory/node-agent-missing-config.txt"

install -m 0755 "$controller_binary" "$evidence_directory/artifacts/kontainer-controller"
install -m 0755 "$node_agent_binary" "$evidence_directory/artifacts/kontainer-node-agent"

uname -a > "$evidence_directory/uname.txt"
rustc -Vv > "$evidence_directory/rustc.txt"
cargo -V > "$evidence_directory/cargo.txt"
git rev-parse HEAD > "$evidence_directory/commit.txt"
sha256sum server/Cargo.lock > "$evidence_directory/lockfile-sha256.txt"
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
