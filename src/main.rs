use std::net::SocketAddr;
use tracing::info;
use serde_json::json;

use event_processing_service::telemetry::init_tracing;
use event_processing_service::Telemetry;
use event_processing_service::store::MemoryStore;
use event_processing_service::service::{IngestService, run_processor_pool};
use event_processing_service::domain::event::Event;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing(None, false);
    let addr: SocketAddr = "127.0.0.1:3000".parse()?;
    info!(%addr, "starting background processor");

    let telemetry = Telemetry::new();
    let store = MemoryStore::new();
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);
    let _ingest = IngestService::new(store.clone(), tx.clone(), telemetry.clone());

    // example handler: echo payload unless payload contains {"fail": true}
    let handler = |ev: Event| async move {
        if ev.payload.0.get("fail").and_then(|v| v.as_bool()).unwrap_or(false) {
            Err("simulated failure".to_string())
        } else {
            Ok(json!({"echo": ev.payload.0}).into())
        }
    };

    let shared_rx = std::sync::Arc::new(tokio::sync::Mutex::new(rx));
    run_processor_pool(store.clone(), shared_rx, tx.clone(), 
    4, // instead of num_cpus::get().max(2), 
    5, telemetry.clone(), handler);

    // keep running until ctrl-c
    tokio::signal::ctrl_c().await?;
    info!("shutting down");
    Ok(())
}
