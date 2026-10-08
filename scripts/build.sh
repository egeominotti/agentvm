#!/bin/bash
# Compila l'helper Swift (firmato con l'entitlement per le VM) e il server Rust in bin/.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p bin
# Sempre un file nuovo + rename: sovrascrivere un binario firmato sullo stesso inode
# fa uccidere da macOS (SIGKILL) i processi avviati dopo.
swiftc -O vm-helper/*.swift -o bin/agentvm-vm.new
codesign -s - -f --entitlements vm-helper/vz.entitlements bin/agentvm-vm.new
mv -f bin/agentvm-vm.new bin/agentvm-vm
cargo build --release --quiet --manifest-path server/Cargo.toml
cp server/target/release/agentvm-server bin/agentvm-server.new
mv -f bin/agentvm-server.new bin/agentvm-server
echo "build ok: bin/agentvm-vm bin/agentvm-server"
