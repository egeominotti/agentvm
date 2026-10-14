#!/bin/bash
# Copy the signed app into a compressed installer image without changing the source bundle.
set -euo pipefail
if [ "$#" -ne 2 ]; then
  echo 'usage: scripts/package-dmg.sh <signed-app> <output.dmg>' >&2
  exit 2
fi
APP=$1
OUTPUT=$2
[ -d "$APP/Contents" ] || { echo "application bundle is missing: $APP" >&2; exit 1; }
codesign --verify --deep --strict "$APP"
STAGING=$(mktemp -d "${TMPDIR:-/tmp}/agentvm-dmg.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT
mkdir "$STAGING/content"
ditto "$APP" "$STAGING/content/agentvm Native.app"
ln -s /Applications "$STAGING/content/Applications"
hdiutil create -quiet -volname agentvm -srcfolder "$STAGING/content" -fs HFS+ \
  -format UDZO "$STAGING/agentvm.dmg"
hdiutil verify -quiet "$STAGING/agentvm.dmg"
mkdir -p "$(dirname "$OUTPUT")"
mv -f "$STAGING/agentvm.dmg" "$OUTPUT"
echo "native DMG: $OUTPUT"
