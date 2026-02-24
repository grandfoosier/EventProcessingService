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
