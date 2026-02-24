use crate::domain::event::Event;
use crate::store::MemoryStore;
use crate::Telemetry;
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

/// Run a pool of processor workers that consume event IDs from `rx`, claim
/// events in the `store` and call the provided async `handler` to process
/// events. If the handler returns Err, the event is requeued until
/// `max_retries` attempts.
pub fn run_processor_pool<H, Fut>(
    store: MemoryStore,
    rx: Arc<Mutex<mpsc::Receiver<String>>>,
    tx: mpsc::Sender<String>,
    workers: usize,
    max_retries: u32,
    telemetry: Telemetry,
    handler: H,
)
where
    H: Fn(Event) -> Fut + Send + Sync + 'static + Clone,
    Fut: Future<Output = Result<Value, String>> + Send + 'static,
{
    let handler = Arc::new(handler);
    for _ in 0..workers {
        let rx = rx.clone();
        let store_clone = store.clone();
        let tx_clone = tx.clone();
        let handler_clone = handler.clone();
        let telemetry = telemetry.clone();
        tokio::spawn(async move {
            loop {
                let opt = {
                    let mut lock = rx.lock().await;
                    lock.recv().await
                };
                let id = match opt {
                    Some(id) => id,
                    None => break,
                };
                // we popped one item off the queue
                telemetry.queue_depth.dec();
                // Try to claim
                match store_clone.claim_for_processing(&id).await {
                    Ok(true) => {
                        // fetch event
                        if let Ok(rec) = store_clone.get(&id).await {
                            let ev = rec.event.clone();
                            let start = Instant::now();
                            let res = (handler_clone)(ev).await;
                            let elapsed = start.elapsed();
                            telemetry.processing_hist.observe(elapsed.as_secs_f64());
                            match res {
                                Ok(result) => {
                                    let _ = store_clone.set_result(&id, result).await;
                                    telemetry.events_processed.inc();
                                }
                                Err(err) => {
                                    // record error and requeue if attempts < max_retries
                                    telemetry.events_failed.inc();
                                    let attempts = rec.attempts;
                                    if attempts >= max_retries {
                                        let _ = store_clone.set_failed(&id, err).await;
                                    } else {
                                        let _ = store_clone.set_error_and_mark_received(&id, err).await;
                                        // backoff
                                        let backoff_ms = 100u64.saturating_mul(2u64.saturating_pow((attempts.saturating_sub(1)) as u32));
                                        let tx2 = tx_clone.clone();
                                        let id2 = id.clone();
                                        // When requeueing, increase queue depth
                                        telemetry.queue_depth.inc();
                                        tokio::spawn(async move {
                                            sleep(Duration::from_millis(backoff_ms.max(50))).await;
                                            let _ = tx2.send(id2).await;
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Ok(false) => continue,
                    Err(_) => continue,
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{Event, EventPayload, EventType};
    use chrono::Utc;
    use serde_json::json;
    use tokio::time::sleep;

    #[tokio::test]
    async fn processor_retries_and_fails() {
        let store = MemoryStore::new();
        let (tx, rx) = mpsc::channel::<String>(16);
        let tx_for_ingest = tx.clone();

        // handler always fails
        let handler = |_: Event| async move { Err("boom".to_string()) };

        // start processor pool
        let shared_rx = Arc::new(Mutex::new(rx));
        let telemetry = Telemetry::new();
        run_processor_pool(store.clone(), shared_rx, tx.clone(), 2, 3, telemetry, handler);

        // insert event via store directly
        let ev = Event {
            event_id: "s1".to_string(),
            event_type: EventType::UserLoginFailed,
            occurred_at: Utc::now(),
            payload: EventPayload(json!({})),
        };
        let (_rec, inserted) = store.insert_if_absent(ev.clone()).await;
        assert!(inserted);
        // enqueue
        let _ = tx_for_ingest.send(ev.event_id.clone()).await;

        // allow retries to exhaust
        sleep(Duration::from_millis(1000)).await;
        let rec = store.get(&ev.event_id).await.unwrap();
        assert_eq!(rec.status, crate::domain::state::EventStatus::Failed);
        assert!(rec.attempts >= 3);
    }
}
