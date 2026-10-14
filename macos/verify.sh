#!/bin/bash
# Exercise a separate native app and real engine without restarting an existing profile.
set -euo pipefail
cd "$(dirname "$0")/.."
QA_REPO_ROOT=$PWD
QA_PORT=7797
QA_TIMEOUT=240
QA_BUILD=1
QA_LAYOUTS_ONLY=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --port) QA_PORT=${2:?missing port}; shift 2 ;;
    --timeout) QA_TIMEOUT=${2:?missing timeout}; shift 2 ;;
    --no-build) QA_BUILD=0; shift ;;
    --layouts-only) QA_LAYOUTS_ONLY=1; shift ;;
    --help)
      echo "usage: macos/verify.sh [--port 7797] [--timeout 240] [--no-build] [--layouts-only]"
      echo "Captures light/dark layouts and checks one real VM terminal component."
      echo "--layouts-only captures without starting a VM; AGENTVM_NATIVE_QA_ROUTES selects routes."
      exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
[[ "$QA_PORT" =~ ^[0-9]+$ ]] && [ "$QA_PORT" -gt 0 ] && [ "$QA_PORT" -le 65535 ] && [ "$QA_PORT" != 7777 ] \
  || { echo "use an isolated port between 1 and 65535, never 7777" >&2; exit 2; }
QA_PORT=$((10#$QA_PORT))
[ "$QA_PORT" != 7777 ] || { echo "verification never uses port 7777" >&2; exit 2; }
[[ "$QA_TIMEOUT" =~ ^[0-9]+$ ]] && [ "$QA_TIMEOUT" -ge 10 ] \
  || { echo "timeout must be at least 10 seconds" >&2; exit 2; }
[ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] \
  || { echo "native verification requires an Apple Silicon Mac" >&2; exit 1; }
python3 - "$QA_PORT" <<'PY'
import socket, sys
with socket.socket() as listener:
    try: listener.bind(('127.0.0.1', int(sys.argv[1])))
    except OSError: sys.exit('The verification port is occupied; choose another port.')
PY
QA_GOLDEN=${AGENTVM_NATIVE_GOLDEN_HOME:-${AGENTVM_HOME:-$HOME/AgentVMs}/golden}
[ -f "$QA_GOLDEN/disk.raw" ] \
  || { echo "golden image missing; set AGENTVM_NATIVE_GOLDEN_HOME to an existing golden folder" >&2; exit 1; }
QA_GOLDEN=$(cd "$QA_GOLDEN" && pwd -P)
QA_RUN_HOME=$(mktemp -d /tmp/agentvm-native-verify.XXXXXX)
QA_CAPTURE_DIR="$QA_RUN_HOME/captures"
QA_APP="$QA_RUN_HOME/agentvm Verification.app"
QA_BUNDLE_ID="com.agentvm.native.verify.$QA_PORT.$(basename "$QA_RUN_HOME" | tr -cd '[:alnum:]')"
QA_APP_PID=
QA_ENGINE_PID=
mkdir -p "$QA_RUN_HOME/golden" "$QA_CAPTURE_DIR"
ln -s "$QA_GOLDEN/disk.raw" "$QA_RUN_HOME/golden/disk.raw"
if [ -d "$QA_GOLDEN/boot" ]; then ln -s "$QA_GOLDEN/boot" "$QA_RUN_HOME/golden/boot"; fi
if [ "$QA_BUILD" = 1 ]; then
  AGENTVM_NATIVE_QA_BUILD=1 AGENTVM_NATIVE_OUTPUT="$QA_APP" macos/build.sh
else
  QA_SOURCE_APP=${AGENTVM_NATIVE_OUTPUT:-$QA_REPO_ROOT/macos/dist/agentvm Native.app}
  [ -x "$QA_SOURCE_APP/Contents/MacOS/AgentVMNative" ] \
    || { echo "native QA app missing; build with AGENTVM_NATIVE_QA_BUILD=1 first" >&2; exit 1; }
  ditto "$QA_SOURCE_APP" "$QA_APP"
fi
[ "$(/usr/libexec/PlistBuddy -c 'Print :AgentVMNativeQAEnabled' "$QA_APP/Contents/Info.plist" 2>/dev/null || true)" = true ] \
  || { echo "this build has no QA harness; omit --no-build or supply an explicit QA app" >&2; exit 1; }
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $QA_BUNDLE_ID" "$QA_APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName agentvm Verification $QA_PORT" "$QA_APP/Contents/Info.plist"
codesign --force --sign - "$QA_APP" >/dev/null 2>&1

cleanup() {
  QA_EXIT_STATUS=$?
  set +e
  if [ -n "$QA_APP_PID" ] && kill -0 "$QA_APP_PID" 2>/dev/null; then
    kill -TERM "$QA_APP_PID" 2>/dev/null
    wait "$QA_APP_PID" 2>/dev/null
  fi
  if [ -n "$QA_ENGINE_PID" ] && kill -0 "$QA_ENGINE_PID" 2>/dev/null; then
    python3 - "$QA_PORT" <<'PY'
import json, sys, urllib.request
try:
    with urllib.request.urlopen('http://127.0.0.1:' + sys.argv[1] + '/api/tasks', timeout=3) as response:
        tasks = json.load(response)
    ended = {'done', 'no_changes', 'failed', 'stopped'}
    sys.exit(0 if all(task.get('status', {}).get('state') in ended for task in tasks) else 1)
except Exception:
    sys.exit(1)
PY
    if [ "$?" = 0 ]; then
      kill -TERM "$QA_ENGINE_PID" 2>/dev/null
      wait "$QA_ENGINE_PID" 2>/dev/null
    else
      echo "Engine left running on :$QA_PORT because live or unknown machines must be preserved."
    fi
  fi
  defaults delete "$QA_BUNDLE_ID" >/dev/null 2>&1
  echo "Verification evidence: $QA_RUN_HOME"
  return "$QA_EXIT_STATUS"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
memory_pressure >"$QA_RUN_HOME/memory-pressure.txt"
if command -v rg >/dev/null 2>&1; then
  rg 'System-wide memory free percentage|Swapins:|Swapouts:' "$QA_RUN_HOME/memory-pressure.txt"
else
  awk '/System-wide memory free percentage|Swapins:|Swapouts:/' "$QA_RUN_HOME/memory-pressure.txt"
fi
QA_RUNTIME="$QA_APP/Contents/Resources/runtime"
QA_ENGINE_PID=$(AGENTVM_HOME="$QA_RUN_HOME" AGENTVM_PORT="$QA_PORT" AGENTVM_CPUS=1 AGENTVM_MEMORY_MB=2048 AGENTVM_CONCURRENCY=1 \
  python3 - "$QA_RUNTIME/bin/agentvm-server" "$QA_RUN_HOME/engine.log" <<'PY'
import subprocess, sys
with open(sys.argv[2], 'ab', buffering=0) as log:
    process = subprocess.Popen([sys.argv[1]], stdin=subprocess.DEVNULL,
                               stdout=log, stderr=log, start_new_session=True)
print(process.pid)
PY
)
QA_READY=0
for _ in $(seq 100); do
  kill -0 "$QA_ENGINE_PID" 2>/dev/null || { cat "$QA_RUN_HOME/engine.log" >&2; exit 1; }
  if curl --connect-timeout 1 --max-time 2 -fsS "http://127.0.0.1:$QA_PORT/api/status" >/dev/null 2>&1; then
    QA_READY=1; break
  fi
  sleep 0.1
done
[ "$QA_READY" = 1 ] || { echo "isolated engine did not become ready" >&2; exit 1; }
AGENTVM_HOME="$QA_RUN_HOME" AGENTVM_PORT="$QA_PORT" AGENTVM_NATIVE_QA_DIRECTORY="$QA_CAPTURE_DIR" \
  AGENTVM_NATIVE_QA_LAYOUTS_ONLY="$QA_LAYOUTS_ONLY" \
  "$QA_APP/Contents/MacOS/AgentVMNative" >"$QA_RUN_HOME/app.log" 2>&1 &
QA_APP_PID=$!
echo "Verifying the separate app on :$QA_PORT with data in $QA_RUN_HOME"
QA_STARTED=$SECONDS
while [ $((SECONDS - QA_STARTED)) -lt "$QA_TIMEOUT" ]; do
  if [ -f "$QA_CAPTURE_DIR/failure.txt" ]; then cat "$QA_CAPTURE_DIR/failure.txt" >&2; exit 1; fi
  if [ -f "$QA_CAPTURE_DIR/result.txt" ]; then cat "$QA_CAPTURE_DIR/result.txt"; exit 0; fi
  kill -0 "$QA_APP_PID" 2>/dev/null || { echo "verification app exited before reporting a result" >&2; exit 1; }
  sleep 1
done
echo "Native verification exceeded $QA_TIMEOUT seconds; inspect the retained evidence." >&2
exit 1
