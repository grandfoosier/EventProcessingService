use crate::domain::event::{Event, EventRecord};
use crate::store::MemoryStore;
use crate::Telemetry;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct IngestService {
    pub store: MemoryStore,
    pub tx: mpsc::Sender<String>,
    pub telemetry: Telemetry,
}

impl IngestService {
    pub fn new(store: MemoryStore, tx: mpsc::Sender<String>, telemetry: Telemetry) -> Self {
        Self { store, tx, telemetry }
    }

    /// Idempotent ingest: insert if absent, enqueue if newly inserted.
    pub async fn ingest(&self, event: Event) -> (EventRecord, bool) {
        let (rec, inserted) = self.store.insert_if_absent(event).await;
        if inserted {
            self.telemetry.events_ingested.inc();
            let _ = self.tx.send(rec.event.event_id.clone()).await;
            self.telemetry.queue_depth.inc();
        } else {
            self.telemetry.events_deduped.inc();
        }
        (rec, inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;
    use crate::telemetry::Telemetry;
    use crate::domain::event::{Event, EventPayload, EventType};
    use chrono::Utc;
    use serde_json::json;

    #[tokio::test]
    async fn ingest_inserts_and_enqueues() {
        let store = MemoryStore::new();
        let telemetry = Telemetry::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(8);
        let svc = IngestService::new(store.clone(), tx, telemetry.clone());

        let ev = Event {
            event_id: "i1".to_string(),
            event_type: EventType::UserLoginFailed,
            occurred_at: Utc::now(),
            payload: EventPayload(json!({"u":"1"})),
        };

        let (rec, inserted) = svc.ingest(ev.clone()).await;
        assert!(inserted);
        assert_eq!(rec.event.event_id, "i1");

        // ensure the id was enqueued
        let queued = rx.recv().await;
        assert_eq!(queued.unwrap(), "i1".to_string());

        // telemetry assertions — queue depth should have increased by the enqueue
        assert!(telemetry.events_ingested.get() > 0);
        assert_eq!(telemetry.queue_depth.get() as i64, 1);
    }

    #[tokio::test]
    async fn ingest_is_idempotent() {
        let store = MemoryStore::new();
        let telemetry = Telemetry::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(8);
        let svc = IngestService::new(store.clone(), tx.clone(), telemetry.clone());

        let ev = Event {
            event_id: "i2".to_string(),
            event_type: EventType::UserLoginFailed,
            occurred_at: Utc::now(),
            payload: EventPayload(json!({})),
        };

        let (_rec1, ins1) = svc.ingest(ev.clone()).await;
        assert!(ins1);
        let (_rec2, ins2) = svc.ingest(ev.clone()).await;
        assert!(!ins2);

        // only one message should be in queue (first insert)
        let q1 = rx.recv().await;
        assert_eq!(q1.unwrap(), "i2".to_string());
        // second recv should be None or timeout; channel empty now
        let q2 = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
        assert!(q2.is_err(), "expected no second message in queue");

        // telemetry assertions
        assert!(telemetry.events_ingested.get() > 0);
        assert!(telemetry.events_deduped.get() > 0);
    }
}
