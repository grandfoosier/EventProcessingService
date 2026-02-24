use event_processing_service::service::IngestService;
use event_processing_service::service::run_processor_pool;
use event_processing_service::store::MemoryStore;
use event_processing_service::telemetry::Telemetry;
use event_processing_service::domain::event::{Event, EventPayload, EventType};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn integration_ingest_and_process() {
    let store = MemoryStore::new();
    let telemetry = Telemetry::new();
    let (tx, rx) = mpsc::channel::<String>(32);
    let ingest = IngestService::new(store.clone(), tx.clone(), telemetry.clone());

    // handler: succeed unless payload contains {"fail": true}
    let handler = |ev: Event| async move {
        if ev.payload.0.get("fail").and_then(|b| b.as_bool()).unwrap_or(false) {
            Err("simulated".to_string())
        } else {
            Ok(json!({"ok": true}))
        }
    };

    let shared_rx = Arc::new(Mutex::new(rx));
    run_processor_pool(store.clone(), shared_rx, tx.clone(), 2, 3, telemetry.clone(), handler);

    // ingest an event
    let ev = Event {
        event_id: "itest-1".to_string(),
        event_type: EventType::UserLoginFailed,
        occurred_at: Utc::now(),
        payload: EventPayload(json!({ "user": "u1" })),
    };
    let (_rec, inserted) = ingest.ingest(ev.clone()).await;
    assert!(inserted);

    // ingest duplicate
    let (_rec2, inserted2) = ingest.ingest(ev.clone()).await;
    assert!(!inserted2);

    // wait for processing
    sleep(Duration::from_millis(500)).await;
    let got = store.get(&ev.event_id).await.expect("record should exist");
    assert_eq!(got.status, event_processing_service::domain::state::EventStatus::Completed);
    assert!(telemetry.events_ingested.get() > 0);
    assert!(telemetry.events_deduped.get() > 0);
}
