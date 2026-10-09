#!/bin/bash
# Creates ~/AgentVMs/golden/disk.raw: Debian 13 arm64 + Claude Code + job runner. One-off (~3 min).
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

[ -x "$HELPER" ] || { echo "$HELPER is missing: run scripts/build.sh first" >&2; exit 1; }
mkdir -p "$IMAGES" "$GOLDEN"

if [ ! -f "$IMAGES/disk.raw" ]; then
  echo "downloading $TARBALL…"
  curl -fsSL -o "$IMAGES/$TARBALL" "$BASE_URL/$TARBALL"
  SUM=$(curl -fsSL "$BASE_URL/SHA512SUMS" | awk -v f="$TARBALL" '$2 == f {print $1}')
  echo "$SUM  $IMAGES/$TARBALL" | shasum -a 512 -c -
  # Into a folder of its own, then renamed: an interrupted extraction never leaves a truncated
  # disk.raw that later builds would take as complete.
  rm -rf "$IMAGES/extract"
  mkdir -p "$IMAGES/extract"
  tar -xJf "$IMAGES/$TARBALL" -C "$IMAGES/extract"
  mv -f "$IMAGES/extract/disk.raw" "$IMAGES/disk.raw"
  rm -rf "$IMAGES/extract"
fi

rm -rf "${BUILD:?}"
mkdir -p "$BUILD/share" "$BUILD/seed"
chmod 777 "$BUILD/share"
cp -R guest/agentvm-boot guest/agentvm-job guest/agentvm-pty guest/agentvm-metrics guest/agentvm-statusline guest/agentvm-claude guest/config guest/agentvm.service guest/10-agentvm.network guest/setup-golden.sh "$BUILD/share/"
CLAUDE_VERSION=${AGENTVM_CLAUDE_VERSION:-latest}
[[ "$CLAUDE_VERSION" =~ ^(latest|stable|[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?)$ ]] || { echo "invalid AGENTVM_CLAUDE_VERSION: $CLAUDE_VERSION" >&2; exit 1; }
printf '%s' "$CLAUDE_VERSION" > "$BUILD/share/claude-version"
echo "Claude Code version for the image: $CLAUDE_VERSION"
cp guest/golden-user-data.yaml "$BUILD/seed/user-data"
printf 'instance-id: agentvm-golden-%s\nlocal-hostname: agentvm\n' "$(date +%s)" > "$BUILD/seed/meta-data"
hdiutil makehybrid -quiet -iso -joliet -default-volume-name cidata -o "$BUILD/seed.iso" "$BUILD/seed"

cp -c "$IMAGES/disk.raw" "$BUILD/disk.raw"
python3 -c 'import os,sys; os.truncate(sys.argv[1], 20 << 30)' "$BUILD/disk.raw"

cat > "$BUILD/vm.json" <<JSON
{"disk":"$BUILD/disk.raw","efivars":"$BUILD/efivars","share":"$BUILD/share",
 "console":"$BUILD/console.log","cpus":4,"memory_mb":4096,"seed_iso":"$BUILD/seed.iso"}
JSON
echo "preparing the golden image (installing packages and Claude Code in the VM)…"
"$HELPER" --config "$BUILD/vm.json"

grep -q GOLDEN_OK "$BUILD/share/setup.log" || { echo "setup failed, see $BUILD/share/setup.log" >&2; exit 1; }
for f in vmlinuz initrd.img cmdline; do
  [ -s "$BUILD/share/boot/$f" ] || { echo "setup left no $f in share/boot" >&2; exit 1; }
done

# A clone boots straight into the kernel once, before any VM does: with nothing to do, the guest's
# job writes its result in the shared folder and powers off. A result there means the initrd found
# the disk and the kernel its modules (virtiofs). Otherwise the image goes without a kernel beside
# it, and VMs boot through EFI and GRUB as they always did.
direct_boot_works() {
  local check=$BUILD/check boot=$BUILD/share/boot pid ok=1
  rm -rf "$check"
  mkdir -p "$check/share"
  chmod 777 "$check/share"
  cp -c "$BUILD/disk.raw" "$check/disk.raw"
  cat > "$check/vm.json" <<JSON
{"disk":"$check/disk.raw","efivars":"$check/efivars","share":"$check/share",
 "console":"$check/console.log","cpus":2,"memory_mb":2048,
 "kernel":"$boot/vmlinuz","initrd":"$boot/initrd.img","cmdline":"$(cat "$boot/cmdline")"}
JSON
  "$HELPER" --config "$check/vm.json" >/dev/null 2>&1 &
  pid=$!
  for _ in $(seq 1 600); do
    [ -s "$check/share/result.json" ] && { ok=0; break; }
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  kill "$pid" 2>/dev/null
  wait "$pid" 2>/dev/null
  rm -rf "$check"
  return $ok
}
DIRECT=1
direct_boot_works || { DIRECT=0; echo "a clone did not boot straight into the kernel: VMs boot through EFI" >&2; }

# The image's kernel goes beside it with the identity of the disk it was built with. The old
# kernel goes first: a VM launched meanwhile boots through EFI, never a kernel with a disk whose
# modules are not its own.
rm -rf "$GOLDEN/boot"
mv "$BUILD/disk.raw" "$GOLDEN/disk.raw"
if [ "$DIRECT" = 1 ]; then
  mv "$BUILD/share/boot" "$BUILD/boot"
  stat -L -f '%i %m' "$GOLDEN/disk.raw" > "$BUILD/boot/disk-id"
  mv "$BUILD/boot" "$GOLDEN/boot"
fi
echo "GOLDEN_OK: $GOLDEN/disk.raw"
