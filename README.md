# Event Processing Service (Rust)

Design: lightweight in-memory event processing service with idempotent ingestion, async workers, retries, metrics and structured logging.

Key points:
- HTTP API: POST /events, GET /events/{id}, GET /healthz, GET /metrics
- In-memory store: `HashMap` protected by `RwLock` inside `AppState`
- Async workers: `tokio` tasks consume an `mpsc` queue
- State transitions: `Received` → `Processing` → `Completed` | `Failed`
- Retries: exponential-ish backoff with capped attempts (`MAX_RETRIES`)
- Tracing: `tracing` + JSON output; request IDs via `x-request-id` header
- Metrics: Prometheus counters exported at `/metrics`

Run:

1. cargo run

Tests:

1. cargo test

Tradeoffs / Notes:
- Data is only in-memory: restarting the service loses state. For production persistence, swap the store for Redis or DB.
- The worker uses a best-effort in-memory retry queue; for durable retries use a persistent queue.
- The example processing is deterministic: include `{"fail": true}` in event payload to simulate failure and retries.

## Smoke Tests

Run quick smoke tests (Windows PowerShell) that build the project, start the server,
check health and metrics, POST a test event, verify its status, and stop the server.

- Script: [scripts/smoke_test.ps1](scripts/smoke_test.ps1)

Usage:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke_test.ps1
```

Optional: specify a start delay (seconds) before the checks:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\smoke_test.ps1 -StartDelaySeconds 2
```

Manual curl examples (linux / WSL / Git Bash):

```bash
curl http://127.0.0.1:3000/healthz
curl http://127.0.0.1:3000/metrics | head -n 20
curl -XPOST -H "Content-Type: application/json" \
  -d '{"event_id":"smoke-1","event_type":"smoke","occurred_at":"2026-02-25T15:07:28Z","payload":{"foo":"bar"}}' \
  http://127.0.0.1:3000/events
curl http://127.0.0.1:3000/events/smoke-1
```

Notes:
- The included `scripts/smoke_test.ps1` is designed for Windows PowerShell. On other platforms run the manual curl commands or translate the script to bash.
