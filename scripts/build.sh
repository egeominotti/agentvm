#!/bin/bash
# Builds the Swift helper (signed with the VM entitlement), the dashboard (web/, embedded in the
# server) and the Rust server into bin/. Only what changed is rebuilt: the helper and the
# dashboard side by side, then the server (cargo is incremental). --force rebuilds everything.
set -euo pipefail
cd "$(dirname "$0")/.."
# agentvm is arm64 only: Apple Silicon host, arm64 Linux guests, no Rosetta anywhere.
[ "$(uname -m)" = arm64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 0 ] ||
  { echo "agentvm builds and runs only natively on Apple Silicon (arm64), not under Rosetta" >&2; exit 1; }
FORCE=${1:-}
mkdir -p bin

# True when `out` exists and nothing in the inputs is newer than it.
fresh() {
  local out=$1
  shift
  [ "$FORCE" != --force ] && [ -e "$out" ] && [ -z "$(find "$@" -newer "$out" -print -quit 2>/dev/null)" ]
}

helper() {
  fresh bin/agentvm-vm vm-helper && return 0
  # Always a new file + rename: overwriting a signed binary on the same inode
  # makes macOS kill (SIGKILL) the processes started afterwards.
  swiftc -O -target "arm64-apple-macos$(sw_vers -productVersion | cut -d. -f1)" vm-helper/*.swift -o bin/agentvm-vm.new
  codesign -s - -f --entitlements vm-helper/vz.entitlements bin/agentvm-vm.new
  mv -f bin/agentvm-vm.new bin/agentvm-vm
}

dashboard() {
  [ -d web/node_modules ] && fresh web/dist/index.html web/src web/public web/index.html web/package.json \
    web/bun.lock web/vite.config.ts web/tsconfig.json && return 0
  (cd web && bun install --frozen-lockfile --silent && bun run --silent build >/dev/null)
}

# The helper and the dashboard do not depend on each other: built at the same time.
helper &
H=$!
dashboard &
D=$!
wait $H || { echo "the VM helper (vm-helper/) did not build" >&2; exit 1; }
wait $D || { echo "the dashboard (web/) did not build" >&2; exit 1; }

cargo build --release --quiet --target aarch64-apple-darwin --manifest-path server/Cargo.toml
# Replaced only when it changed: a server already running from it is not restarted for nothing.
if ! cmp -s server/target/aarch64-apple-darwin/release/agentvm-server bin/agentvm-server; then
  cp server/target/aarch64-apple-darwin/release/agentvm-server bin/agentvm-server.new
  mv -f bin/agentvm-server.new bin/agentvm-server
fi
for b in bin/agentvm-vm bin/agentvm-server; do
  [ "$(lipo -archs "$b")" = arm64 ] || { echo "$b is not a pure arm64 binary: $(lipo -archs "$b")" >&2; exit 1; }
done
echo "build ok (arm64): bin/agentvm-vm bin/agentvm-server"
