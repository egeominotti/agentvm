#!/bin/bash
# Local S3-compatible server (RustFS in Docker) for trying backups end to end.
#   scripts/dev-s3.sh up      start it and create the bucket
#   scripts/dev-s3.sh down    stop it (data kept in a Docker volume)
#   scripts/dev-s3.sh reset   stop it and delete its data
set -euo pipefail
cd "$(dirname "$0")/.."
COMPOSE=(docker compose -f dev/s3/docker-compose.yml)
ENDPOINT=http://127.0.0.1:9100
BUCKET=agentvm-backups
ACCESS_KEY=agentvm
SECRET_KEY=agentvm-local-secret

case "${1:-up}" in
  up)
    "${COMPOSE[@]}" up -d
    for _ in $(seq 60); do
      [ "$(curl -s -o /dev/null -w '%{http_code}' "$ENDPOINT/")" != "000" ] && break
      sleep 0.5
    done
    printf 'user = "%s:%s"\n' "$ACCESS_KEY" "$SECRET_KEY" \
      | curl -s -K - --aws-sigv4 "aws:amz:us-east-1:s3" -X PUT -o /dev/null "$ENDPOINT/$BUCKET"
    cat <<INFO
S3 is up. Use these values in agentvm → Settings → Backups to S3:
  Provider      Local (RustFS / MinIO)
  Endpoint      $ENDPOINT
  Region        us-east-1
  Bucket        $BUCKET
  Access key    $ACCESS_KEY
  Secret key    $SECRET_KEY
  Path-style    on
Console: http://127.0.0.1:9101
INFO
    ;;
  down) "${COMPOSE[@]}" down ;;
  reset) "${COMPOSE[@]}" down -v ;;
  *) echo "usage: $0 [up|down|reset]" >&2; exit 2 ;;
esac
