#!/bin/bash
# Builds the Swift helper (signed with the VM entitlement) and the Rust server into bin/.
set -euo pipefail
cd "$(dirname "$0")/.."
# agentvm is arm64 only: Apple Silicon host, arm64 Linux guests, no Rosetta anywhere.
[ "$(uname -m)" = arm64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 0 ] ||
  { echo "agentvm builds and runs only natively on Apple Silicon (arm64), not under Rosetta" >&2; exit 1; }
mkdir -p bin
# Always a new file + rename: overwriting a signed binary on the same inode
# makes macOS kill (SIGKILL) the processes started afterwards.
swiftc -O -target "arm64-apple-macos$(sw_vers -productVersion | cut -d. -f1)" vm-helper/*.swift -o bin/agentvm-vm.new
codesign -s - -f --entitlements vm-helper/vz.entitlements bin/agentvm-vm.new
mv -f bin/agentvm-vm.new bin/agentvm-vm
cargo build --release --quiet --target aarch64-apple-darwin --manifest-path server/Cargo.toml
cp server/target/aarch64-apple-darwin/release/agentvm-server bin/agentvm-server.new
mv -f bin/agentvm-server.new bin/agentvm-server
for b in bin/agentvm-vm bin/agentvm-server; do
  [ "$(lipo -archs "$b")" = arm64 ] || { echo "$b is not a pure arm64 binary: $(lipo -archs "$b")" >&2; exit 1; }
done
echo "build ok (arm64): bin/agentvm-vm bin/agentvm-server"
