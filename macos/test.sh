#!/bin/bash
# Native unit tests; optional integration uses a separate real engine supplied by the caller.
set -euo pipefail
cd "$(dirname "$0")/.."
source macos/swift-env.sh
NATIVE_SWIFT_MAJOR=$(swift --version | sed -n 's/.*Swift version \([0-9]*\).*/\1/p' | head -1)
[ "${NATIVE_SWIFT_MAJOR:-0}" -ge 6 ] || { echo "native tests require Swift 6 or newer; the app supports Swift 5.9" >&2; exit 1; }
NATIVE_TEST_OPTIONS=(--disable-xctest --enable-swift-testing)
NATIVE_TEST_HELP=$(swift test --package-path macos --help)
if [[ "$NATIVE_TEST_HELP" == *swiftbuild* ]]; then
  # The new build backend cannot load macros compiled for the preview SDK with a stable SDK.
  NATIVE_TEST_OPTIONS+=(--build-system native)
fi
if [[ "$NATIVE_TEST_HELP" == *disable-experimental-prebuilts* ]]; then
  NATIVE_TEST_OPTIONS+=(--disable-experimental-prebuilts)
fi
NATIVE_TESTING_LIBS="$(xcode-select -p)/Library/Developer/usr/lib"
if [ ! -f "$NATIVE_TESTING_LIBS/lib_TestingInterop.dylib" ]; then
  NATIVE_TESTING_LIBS="$(xcode-select -p)/usr/lib"
fi
if [ -f "$NATIVE_TESTING_LIBS/lib_TestingInterop.dylib" ]; then
  NATIVE_TEST_OPTIONS+=(-Xlinker -L -Xlinker "$NATIVE_TESTING_LIBS" -Xlinker -rpath -Xlinker "$NATIVE_TESTING_LIBS")
fi
bun macos/build-terminal.ts
bun test macos/Tests/preview-screen.test.ts
mkdir -p macos/Sources/AgentVMNative/Resources/fonts
cp web/public/fonts/* macos/Sources/AgentVMNative/Resources/fonts/
swift test --package-path macos --sdk "$NATIVE_SDK" "${NATIVE_TEST_OPTIONS[@]}" "$@"
