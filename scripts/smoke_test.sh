#!/usr/bin/env bash
set -euo pipefail

START_DELAY=${1:-1}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

echo "Building project..."
cargo build

EXE="$REPO_ROOT/target/debug/event_processing_service"
if [ ! -x "$EXE" ]; then
  echo "Executable not found: $EXE" >&2
  exit 1
fi

echo "Starting server..."
"$EXE" &
PID=$!
trap 'echo "Stopping server..."; kill "$PID" 2>/dev/null || true' EXIT

echo "Waiting for server to become ready..."
for i in $(seq 1 20); do
  if curl -sS http://127.0.0.1:3000/healthz >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

echo
echo "GET /healthz"
curl -sS http://127.0.0.1:3000/healthz || true
echo

echo "GET /metrics (first 20 lines)"
curl -sS http://127.0.0.1:3000/metrics | head -n 20 || true
echo

echo "POST /events"
TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
read -r -d '' PAYLOAD <<EOF || true
{"event_id":"smoke-1","event_type":"smoke","occurred_at":"$TS","payload":{"foo":"bar"}}
EOF

curl -sS -XPOST -H "Content-Type: application/json" -d "$PAYLOAD" http://127.0.0.1:3000/events || true
echo

sleep 1

echo "GET /events/smoke-1"
curl -sS http://127.0.0.1:3000/events/smoke-1 || true
echo

echo "Done."
