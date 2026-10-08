#!/bin/bash
# Runs every test binary at the same time (cargo runs binaries one after another; inside each,
# tests already run on all cores). `scripts/test.sh` for the fast suite, `--ignored` for real VMs,
# Claude and S3, `--all` for both.
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
pids=()
for b in $bins; do
  name=$(basename "$b" | sed 's/-[0-9a-f]*$//')
  "$b" ${args[@]+"${args[@]}"} > "$logs/$name.log" 2>&1 &
  pids+=("$!:$name")
done
fail=0
for p in "${pids[@]}"; do
  pid=${p%%:*} name=${p#*:}
  if wait "$pid"; then
    printf '  ok    %-14s %s\n' "$name" "$(grep -h 'test result' "$logs/$name.log" | tail -1 | sed 's/test result: //')"
  else
    fail=1
    printf '  FAIL  %s\n' "$name"
    grep -E '^(test .* FAILED|---- |thread .* panicked)' "$logs/$name.log" | head -20 | sed 's/^/        /'
  fi
done
echo "logs: $logs"
exit $fail
