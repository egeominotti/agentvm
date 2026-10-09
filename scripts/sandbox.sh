#!/bin/bash
# A second agentvm, isolated from the real one: its own home, its own port, the same VM image.
# For trying changes with real VMs without touching the machines on :7777.
#
#   scripts/sandbox.sh [port]   build what changed, (re)start the sandbox server (default :7790)
#   scripts/sandbox.sh stop     stop its VMs, then the server
#
# The image is shared through a symlink to ~/AgentVMs/golden/disk.raw: the server refuses to boot
# a disk that is a link (see JobWorkspace::check_own_disk), so the real image is never written.
set -euo pipefail
cd "$(dirname "$0")/.."
SANDBOX=${AGENTVM_SANDBOX:-$HOME/AgentVMs-sandbox}
GOLDEN=${AGENTVM_HOME:-$HOME/AgentVMs}/golden/disk.raw

server_on() { lsof -t -iTCP:"$1" -sTCP:LISTEN 2>/dev/null | head -1 || true; }

if [ "${1:-}" = stop ]; then
  PORT=$(cat "$SANDBOX/port" 2>/dev/null || echo 7790)
  running=$(curl -fs "http://127.0.0.1:$PORT/api/tasks" | python3 -I -c \
    'import json, sys; print(" ".join(t["id"] for t in json.load(sys.stdin) if t["status"]["state"] in ("queued", "preparing", "booting", "running")))' || true)
  for id in $running; do
    curl -fs -X POST -H "Origin: http://127.0.0.1:$PORT" "http://127.0.0.1:$PORT/api/tasks/$id/stop" >/dev/null && echo "stopped VM $id"
  done
  pid=$(server_on "$PORT")
  [ -n "$pid" ] && kill "$pid" && echo "sandbox on :$PORT stopped"
  exit 0
fi

PORT=${1:-7790}
[ "$PORT" = 7777 ] && { echo "7777 is the real agentvm: pick another port" >&2; exit 1; }
[ -f "$GOLDEN" ] || { echo "no VM image at $GOLDEN: run ./quickstart once" >&2; exit 1; }
mkdir -p "$SANDBOX/golden" "$SANDBOX/logs"
ln -sf "$GOLDEN" "$SANDBOX/golden/disk.raw"
echo "$PORT" > "$SANDBOX/port"
scripts/build.sh >/dev/null
pid=$(server_on "$PORT")
[ -n "$pid" ] && kill "$pid" && sleep 0.5
AGENTVM_HOME="$SANDBOX" AGENTVM_PORT="$PORT" nohup bin/agentvm-server >>"$SANDBOX/logs/server.out" 2>&1 &
for _ in $(seq 1 50); do curl -fs -o /dev/null "http://127.0.0.1:$PORT/api/status" && break; sleep 0.1; done
curl -fs -o /dev/null "http://127.0.0.1:$PORT/api/status" || { tail -20 "$SANDBOX/logs/server.out" >&2; exit 1; }
echo "sandbox: http://127.0.0.1:$PORT  (home $SANDBOX, log $SANDBOX/logs/server.out)"
