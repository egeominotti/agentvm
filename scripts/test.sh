#!/bin/bash
# Runs every test binary at the same time (cargo runs binaries one after another; inside each,
# tests already run on all cores). `scripts/test.sh` for the fast suite, `--ignored` for real VMs,
# Claude and S3, `--all` for both. The binaries that boot VMs run one after another: together they
# would start dozens of VMs at once and fail their timing checks.
set -euo pipefail
cd "$(dirname "$0")/.."
mode=${1:-}
# The dashboard first: the server embeds its build, and its own checks join the report.
(cd web && bun install --frozen-lockfile --silent && bun run --silent build >/dev/null)
cd server
# The binaries run here directly, without Cargo's [env]: say where the API types go (see .cargo/config.toml).
export TS_RS_EXPORT_DIR="$PWD/../web/src/api/generated" TS_RS_LARGE_INT=number
# Compile errors are shown, not hidden: the suite stops here when the code does not build.
cargo test --no-run --quiet
artifacts=$(cargo test --no-run --message-format=json 2>/dev/null)
bins=$(jq -r 'select(.reason == "compiler-artifact" and .profile.test == true) | .executable // empty' <<< "$artifacts")
mcp=$(jq -r 'select(.reason == "compiler-artifact" and .target.name == "agentvm-mcp" and .profile.test == false) | .executable // empty' <<< "$artifacts")
# macOS can defer a fresh executable before main under parallel process load.
# Prepare the actual MCP executable before the stdio test's protocol-response deadline starts.
if [ -n "$mcp" ]; then
  "$mcp" --help </dev/null >/dev/null 2>&1
fi
args=()
case "$mode" in
  --ignored) args=(--ignored) ;;
  --all) args=(--include-ignored) ;;
esac
logs=$(mktemp -d)
report() { # name status
  if [ "$2" = 0 ]; then
    printf '  ok    %-14s %s\n' "$1" "$(grep -hE 'test result|^ +Tests |^Ran .* tests' "$logs/$1.log" | tail -1 | sed -E 's/(test result: |^ +Tests +)//')"
  else
    fail=1
    printf '  FAIL  %s\n' "$1"
    grep -E '^(test .* FAILED|---- |thread .* panicked)' "$logs/$1.log" | head -20 | sed 's/^/        /'
  fi
}
pids=()
(cd ../web && bun run --silent lint && bun run --silent test) > "$logs/web.log" 2>&1 &
pids+=("$!:web")
(cd .. && python3 -m unittest discover -s guest/tests) > "$logs/guest.log" 2>&1 &
pids+=("$!:guest")
vm_bins=()
latency_bin=
for b in $bins; do
  name=$(basename "$b" | sed 's/-[0-9a-f]*$//')
  if [ ${#args[@]} -gt 0 ] && { [ "$name" = vm ] || [ "$name" = system ]; }; then
    vm_bins+=("$b")
    continue
  fi
  if [ "$name" = adapters ] && [ "$mode" != --ignored ]; then
    # Measure command latency without the hundreds of concurrent subprocesses above.
    latency_bin=$b
    "$b" ${args[@]+"${args[@]}"} --skip process::a_quick_command_returns_at_once > "$logs/$name.log" 2>&1 &
  else
    "$b" ${args[@]+"${args[@]}"} > "$logs/$name.log" 2>&1 &
  fi
  pids+=("$!:$name")
done
fail=0
for p in ${pids[@]+"${pids[@]}"}; do
  pid=${p%%:*} name=${p#*:}
  status=0
  wait "$pid" || status=$?
  report "$name" "$status"
done
for b in ${vm_bins[@]+"${vm_bins[@]}"}; do
  name=$(basename "$b" | sed 's/-[0-9a-f]*$//')
  status=0
  "$b" "${args[@]}" > "$logs/$name.log" 2>&1 || status=$?
  report "$name" "$status"
done
if [ -n "$latency_bin" ]; then
  status=0
  "$latency_bin" --exact process::a_quick_command_returns_at_once --test-threads=1 > "$logs/process_latency.log" 2>&1 || status=$?
  report process_latency "$status"
fi
echo "logs: $logs"
exit $fail
