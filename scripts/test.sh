#!/bin/bash
# Runs every test binary at the same time (cargo runs binaries one after another; inside each,
# tests already run on all cores). `scripts/test.sh` for the fast suite, `--ignored` for real VMs,
# Claude and S3, `--all` for both. The binaries that boot VMs run one after another: together they
# would start dozens of VMs at once and fail their timing checks.
set -euo pipefail
cd "$(dirname "$0")/../server"
mode=${1:-}
cargo test --no-run --quiet 2>/dev/null
bins=$(cargo test --no-run --message-format=json 2>/dev/null \
  | jq -r 'select(.reason == "compiler-artifact" and .profile.test == true) | .executable // empty')
args=()
case "$mode" in
  --ignored) args=(--ignored) ;;
  --all) args=(--include-ignored) ;;
esac
logs=$(mktemp -d)
report() { # name status
  if [ "$2" = 0 ]; then
    printf '  ok    %-14s %s\n' "$1" "$(grep -h 'test result' "$logs/$1.log" | tail -1 | sed 's/test result: //')"
  else
    fail=1
    printf '  FAIL  %s\n' "$1"
    grep -E '^(test .* FAILED|---- |thread .* panicked)' "$logs/$1.log" | head -20 | sed 's/^/        /'
  fi
}
pids=()
vm_bins=()
for b in $bins; do
  name=$(basename "$b" | sed 's/-[0-9a-f]*$//')
  if [ ${#args[@]} -gt 0 ] && { [ "$name" = vm ] || [ "$name" = system ]; }; then
    vm_bins+=("$b")
    continue
  fi
  "$b" ${args[@]+"${args[@]}"} > "$logs/$name.log" 2>&1 &
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
echo "logs: $logs"
exit $fail
