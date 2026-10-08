#!/bin/bash
# Compila l'helper Swift (firmato con l'entitlement per le VM) e il server Rust in bin/.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p bin
swiftc -O vm-helper/*.swift -o bin/agentvm-vm
codesign -s - -f --entitlements vm-helper/vz.entitlements bin/agentvm-vm
cargo build --release --quiet --manifest-path server/Cargo.toml
cp server/target/release/agentvm-server bin/agentvm-server
echo "build ok: bin/agentvm-vm bin/agentvm-server"
