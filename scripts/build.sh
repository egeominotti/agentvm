#!/bin/bash
# Builds the Swift helper (signed with the VM entitlement) and the Rust server into bin/.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p bin
# Always a new file + rename: overwriting a signed binary on the same inode
# makes macOS kill (SIGKILL) the processes started afterwards.
swiftc -O vm-helper/*.swift -o bin/agentvm-vm.new
codesign -s - -f --entitlements vm-helper/vz.entitlements bin/agentvm-vm.new
mv -f bin/agentvm-vm.new bin/agentvm-vm
cargo build --release --quiet --manifest-path server/Cargo.toml
cp server/target/release/agentvm-server bin/agentvm-server.new
mv -f bin/agentvm-server.new bin/agentvm-server
echo "build ok: bin/agentvm-vm bin/agentvm-server"
