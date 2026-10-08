#!/bin/bash
# Crea ~/AgentVMs/golden/disk.raw: Debian 13 arm64 + Claude Code + job runner. Una tantum (~2 min).
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
HOME_DIR=${AGENTVM_HOME:-$HOME/AgentVMs}
IMAGES=$HOME_DIR/images
GOLDEN=$HOME_DIR/golden
BUILD=$GOLDEN/build
BASE_URL=https://cloud.debian.org/images/cloud/trixie/latest
TARBALL=debian-13-genericcloud-arm64.tar.xz
HELPER=$ROOT/bin/agentvm-vm

[ -x "$HELPER" ] || { echo "manca $HELPER: esegui prima scripts/build.sh" >&2; exit 1; }
mkdir -p "$IMAGES" "$GOLDEN"

if [ ! -f "$IMAGES/disk.raw" ]; then
  echo "scarico $TARBALL…"
  curl -fsSL -o "$IMAGES/$TARBALL" "$BASE_URL/$TARBALL"
  SUM=$(curl -fsSL "$BASE_URL/SHA512SUMS" | awk -v f="$TARBALL" '$2 == f {print $1}')
  echo "$SUM  $IMAGES/$TARBALL" | shasum -a 512 -c -
  tar -xJf "$IMAGES/$TARBALL" -C "$IMAGES"
fi

rm -rf "${BUILD:?}"
mkdir -p "$BUILD/share" "$BUILD/seed"
chmod 777 "$BUILD/share"
cp guest/agentvm-job guest/agentvm.service guest/10-agentvm.network guest/setup-golden.sh "$BUILD/share/"
cp guest/golden-user-data.yaml "$BUILD/seed/user-data"
printf 'instance-id: agentvm-golden-%s\nlocal-hostname: agentvm\n' "$(date +%s)" > "$BUILD/seed/meta-data"
hdiutil makehybrid -quiet -iso -joliet -default-volume-name cidata -o "$BUILD/seed.iso" "$BUILD/seed"

cp -c "$IMAGES/disk.raw" "$BUILD/disk.raw"
python3 -c 'import os,sys; os.truncate(sys.argv[1], 20 << 30)' "$BUILD/disk.raw"

cat > "$BUILD/vm.json" <<JSON
{"disk":"$BUILD/disk.raw","efivars":"$BUILD/efivars","share":"$BUILD/share",
 "console":"$BUILD/console.log","cpus":4,"memory_mb":4096,"seed_iso":"$BUILD/seed.iso"}
JSON
echo "preparo la golden (installazione di pacchetti e Claude Code nella VM)…"
"$HELPER" --config "$BUILD/vm.json"

grep -q GOLDEN_OK "$BUILD/share/setup.log" || { echo "setup fallito, vedi $BUILD/share/setup.log" >&2; exit 1; }
mv "$BUILD/disk.raw" "$GOLDEN/disk.raw"
echo "GOLDEN_OK: $GOLDEN/disk.raw"
