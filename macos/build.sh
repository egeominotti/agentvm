#!/bin/bash
# An additional native arm64 app: existing source, dashboard and launch scripts stay unchanged.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
source macos/swift-env.sh
[ "$(uname -m)" = arm64 ] || { echo "the native app requires Apple Silicon" >&2; exit 1; }
scripts/build.sh
bun macos/build-terminal.ts
mkdir -p macos/Sources/AgentVMNative/Resources/fonts
cp web/public/fonts/* macos/Sources/AgentVMNative/Resources/fonts/
swift build --package-path macos --sdk "$NATIVE_SDK" -c release --arch arm64
PRODUCTS=$(swift build --package-path macos --sdk "$NATIVE_SDK" -c release --arch arm64 --show-bin-path)
APP=${AGENTVM_NATIVE_OUTPUT:-$ROOT/macos/dist/agentvm Native.app}
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/runtime/bin"
cp "$PRODUCTS/AgentVMNative" "$APP/Contents/MacOS/AgentVMNative.new"
mv -f "$APP/Contents/MacOS/AgentVMNative.new" "$APP/Contents/MacOS/AgentVMNative"
cp macos/Info.plist "$APP/Contents/Info.plist"
if [ "${AGENTVM_NATIVE_QA_BUILD:-0}" = 1 ]; then
  /usr/libexec/PlistBuddy -c 'Add :AgentVMNativeQAEnabled bool true' "$APP/Contents/Info.plist"
fi
cp -R "$PRODUCTS/AgentVMNative_AgentVMNative.bundle" "$APP/Contents/Resources/"
RUNTIME_BINARIES=(agentvm-server agentvm-vm)
if [ -f bin/agentvm-mcp ]; then
  RUNTIME_BINARIES+=(agentvm-mcp)
else
  # A reused bundle must not retain an MCP server from a different engine build.
  bun -e 'const { unlinkSync, existsSync } = require("node:fs"); const p = process.argv[1]; if (existsSync(p)) unlinkSync(p);' "$APP/Contents/Resources/runtime/bin/agentvm-mcp"
fi
for name in "${RUNTIME_BINARIES[@]}"; do
  # Signed Mach-O files must get a new inode; overwriting one invalidates macOS's signature cache.
  cp "bin/$name" "$APP/Contents/Resources/runtime/bin/$name.new"
  mv -f "$APP/Contents/Resources/runtime/bin/$name.new" "$APP/Contents/Resources/runtime/bin/$name"
done
bun macos/package-metadata.ts "$APP"
mkdir -p "$APP/Contents/Resources/runtime/scripts" "$APP/Contents/Resources/runtime/guest"
cp scripts/build-golden.sh "$APP/Contents/Resources/runtime/scripts/"
cp -R guest/. "$APP/Contents/Resources/runtime/guest/"
cp LICENSE "$APP/Contents/Resources/LICENSE"
ICONSET="$ROOT/macos/dist/AppIcon.iconset"
mkdir -p "$ICONSET"
swift macos/make-icon.swift "$ROOT/macos/dist/AppIcon.png"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$ROOT/macos/dist/AppIcon.png" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  retina=$((size * 2))
  sips -z "$retina" "$retina" "$ROOT/macos/dist/AppIcon.png" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
ARM64_BINARIES=("$APP/Contents/MacOS/AgentVMNative")
for name in "${RUNTIME_BINARIES[@]}"; do ARM64_BINARIES+=("$APP/Contents/Resources/runtime/bin/$name"); done
for file in "${ARM64_BINARIES[@]}"; do
  [ "$(lipo -archs "$file")" = arm64 ] || { echo "not a pure arm64 binary: $file" >&2; exit 1; }
done
codesign --force --sign - "$APP/Contents/Resources/runtime/bin/agentvm-server"
if [ -f "$APP/Contents/Resources/runtime/bin/agentvm-mcp" ]; then
  codesign --force --sign - "$APP/Contents/Resources/runtime/bin/agentvm-mcp"
fi
# The helper keeps its virtualization entitlement when copied into the app.
codesign --verify --strict "$APP/Contents/Resources/runtime/bin/agentvm-vm"
codesign --force --sign - "$APP"
codesign --verify --deep --strict "$APP"
AGENTVM_NATIVE_OUTPUT="$APP" bun test macos/Tests/package.test.ts
if [ "${AGENTVM_NATIVE_QA_BUILD:-0}" != 1 ]; then
  ditto -c -k --sequesterRsrc --keepParent "$APP" "$ROOT/macos/dist/agentvm-Native-Apple-Silicon.zip"
fi
echo "native app: $APP"
