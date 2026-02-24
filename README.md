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
# EventProcessingService
A fault-tolerant service to accept and process events through HTTP, build in async Rust (POC)
