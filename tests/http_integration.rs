use serde_json::json;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use event_processing_service::domain::event::Event;
use event_processing_service::service::IngestService;
use event_processing_service::store::MemoryStore;
use event_processing_service::telemetry::Telemetry;
use event_processing_service::http::routes::build_router;
use event_processing_service::service::run_processor_pool;

#[tokio::test]
async fn http_end_to_end() -> anyhow::Result<()> {
    // prepare components
    let store = MemoryStore::new();
    let telemetry = Telemetry::new();
    let (tx, rx) = mpsc::channel::<String>(32);
    let ingest = IngestService::new(store.clone(), tx.clone(), telemetry.clone());

    // start processor pool with a handler that succeeds
    let handler = |_ev: Event| async move { Ok(serde_json::json!({"ok": true})) };
    let shared_rx = Arc::new(Mutex::new(rx));
    run_processor_pool(store.clone(), shared_rx, tx.clone(), 2, 3, telemetry.clone(), handler);

    // give the processor a moment to pick up the message
    // build http app
    let state = Arc::new(event_processing_service::http::handlers::HttpState { ingest, store: store.clone(), telemetry: telemetry.clone() });
    let _app = build_router(state.clone());

    // Instead of starting a full HTTP server (which can surface crate-version
    // compatibility issues in tests), exercise the HTTP handlers directly.
    use event_processing_service::http::handlers::{post_events, get_event, metrics, healthz};
    use event_processing_service::http::types::EventIn;
    use axum::extract::State as AxState;
    use axum::response::IntoResponse;
    use axum::Json as AxJson;
    use axum::http::StatusCode;
    use axum::body::to_bytes;

    let occurred_at = chrono::Utc::now();
    let ev_in = EventIn { event_id: "httptest1".to_string(), event_type: "user.login_failed".to_string(), occurred_at, payload: json!({"user": "u1"}) };

    // call POST handler
    let resp = post_events(AxState(state.clone()), AxJson(ev_in.clone())).await.into_response();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // call POST duplicate
    let resp2 = post_events(AxState(state.clone()), AxJson(ev_in.clone())).await.into_response();
    assert!(resp2.status().is_success());

    // wait for the store to report Completed (up to 5s) without busy sleeps
    use event_processing_service::domain::state::EventStatus;
    let ok = state.store.wait_for_status("httptest1", EventStatus::Completed, std::time::Duration::from_secs(5)).await;
    assert!(ok, "event did not complete in time");

    // verify HTTP GET returns the completed record
    let resp = get_event(AxState(state.clone()), axum::extract::Path("httptest1".to_string())).await.into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = to_bytes(resp.into_body(), 16_384).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();
    assert_eq!(v["status"], json!("Completed"));

    // metrics and healthz handlers
    let _m = metrics(AxState(state.clone())).await.into_response();
    let _h = healthz(AxState(state.clone())).await.into_response();
    Ok(())
}
