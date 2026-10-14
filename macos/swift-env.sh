#!/bin/bash
# Preview SDKs can reference SwiftUI macro plugins not shipped in Command Line Tools.
# Prefer the installed stable SDK; an explicit SDK remains available for Xcode builds.
NATIVE_SDK=${AGENTVM_NATIVE_SDK:-$(xcrun --sdk macosx --show-sdk-path)}
if [ -z "${AGENTVM_NATIVE_SDK:-}" ] && [ -d "$(dirname "$NATIVE_SDK")/MacOSX26.5.sdk" ]; then
  NATIVE_SDK="$(dirname "$NATIVE_SDK")/MacOSX26.5.sdk"
fi
[ -d "$NATIVE_SDK" ] || { echo "native SDK missing: $NATIVE_SDK" >&2; exit 1; }
