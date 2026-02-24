use crate::domain::event::{Event, EventRecord};
use crate::domain::state::EventStatus;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, Notify};

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("not found")]
    NotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{Event, EventPayload, EventType};
    use chrono::Utc;
    use serde_json::json;

    #[tokio::test]
    async fn insert_and_get() {
        let store = MemoryStore::new();
        let ev = Event {
            event_id: "e1".to_string(),
            event_type: EventType::UserLoginFailed,
            occurred_at: Utc::now(),
            payload: EventPayload(json!({"user_id": "u1"})),
        };
        let (_rec, inserted) = store.insert_if_absent(ev.clone()).await;
        assert!(inserted);
        let got = store.get(&ev.event_id).await.unwrap();
        assert_eq!(got.event.event_id, "e1");
        assert_eq!(got.status, EventStatus::Received);

        // idempotent insert
        let (_rec2, inserted2) = store.insert_if_absent(ev).await;
        assert!(!inserted2);
    }

    #[tokio::test]
    async fn claim_and_complete() {
        let store = MemoryStore::new();
        let ev = Event {
            event_id: "e2".to_string(),
            event_type: EventType::UserLoginFailed,
            occurred_at: Utc::now(),
            payload: EventPayload(json!({})),
        };
        let (_rec, _ins) = store.insert_if_absent(ev.clone()).await;
        let claimed = store.claim_for_processing(&ev.event_id).await.unwrap();
        assert!(claimed);
        // second claim should return false because it's Processing now
        let claimed2 = store.claim_for_processing(&ev.event_id).await.unwrap();
        assert!(!claimed2);

        // set result
        store.set_result(&ev.event_id, json!({"ok": true})).await.unwrap();
        let got = store.get(&ev.event_id).await.unwrap();
        assert_eq!(got.status, EventStatus::Completed);
        assert_eq!(got.result.unwrap()["ok"], json!(true));
    }

    #[tokio::test]
    async fn set_failed_marks_failed() {
        let store = MemoryStore::new();
        let ev = Event {
            event_id: "e3".to_string(),
            event_type: EventType::UserLoginFailed,
            occurred_at: Utc::now(),
            payload: EventPayload(json!({})),
        };
        let (_rec, _ins) = store.insert_if_absent(ev.clone()).await;
        store.set_failed(&ev.event_id, "boom".to_string()).await.unwrap();
        let got = store.get(&ev.event_id).await.unwrap();
        assert_eq!(got.status, EventStatus::Failed);
        assert_eq!(got.last_error.unwrap(), "boom");
    }
}

#[derive(Clone)]
pub struct MemoryStore {
    inner: Arc<RwLock<HashMap<String, EventRecord>>>,
    // per-event notifiers for deterministic signaling
    notifiers: Arc<RwLock<HashMap<String, Arc<Notify>>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(HashMap::new())), notifiers: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Insert if absent. Returns true if inserted, false if already existed.
    pub async fn insert_if_absent(&self, event: Event) -> (EventRecord, bool) {
        let mut map = self.inner.write().await;
        if let Some(existing) = map.get(&event.event_id) {
            return (existing.clone(), false);
        }
        let rec = EventRecord::new(event);
        map.insert(rec.event.event_id.clone(), rec.clone());
        // create per-event notifier
        let mut notifs = self.notifiers.write().await;
        notifs.insert(rec.event.event_id.clone(), Arc::new(Notify::new()));
        (rec, true)
    }

    pub async fn get(&self, id: &str) -> Result<EventRecord, StoreError> {
        let map = self.inner.read().await;
        map.get(id).cloned().ok_or(StoreError::NotFound)
    }

    /// Claim for processing: move Received -> Processing and increment attempts atomically.
    pub async fn claim_for_processing(&self, id: &str) -> Result<bool, StoreError> {
        let mut map = self.inner.write().await;
        let rec = map.get_mut(id).ok_or(StoreError::NotFound)?;
        if rec.status == EventStatus::Received {
            rec.status = EventStatus::Processing;
            rec.attempts = rec.attempts.saturating_add(1);
            rec.updated_at = Utc::now();
            // notify per-event watchers that status changed
            if let Some(n) = self.notifiers.read().await.get(id) {
                n.notify_waiters();
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn set_result(&self, id: &str, result: Value) -> Result<(), StoreError> {
        let mut map = self.inner.write().await;
        let rec = map.get_mut(id).ok_or(StoreError::NotFound)?;
        rec.result = Some(result);
        rec.status = EventStatus::Completed;
        rec.updated_at = Utc::now();
        if let Some(n) = self.notifiers.read().await.get(id) {
            n.notify_waiters();
        }
        Ok(())
    }

    pub async fn set_failed(&self, id: &str, err: String) -> Result<(), StoreError> {
        let mut map = self.inner.write().await;
        let rec = map.get_mut(id).ok_or(StoreError::NotFound)?;
        rec.last_error = Some(err);
        rec.status = EventStatus::Failed;
        rec.updated_at = Utc::now();
        if let Some(n) = self.notifiers.read().await.get(id) {
            n.notify_waiters();
        }
        Ok(())
    }

    /// Record an error and move back to `Received` for retry.
    pub async fn set_error_and_mark_received(&self, id: &str, err: String) -> Result<(), StoreError> {
        let mut map = self.inner.write().await;
        let rec = map.get_mut(id).ok_or(StoreError::NotFound)?;
        rec.last_error = Some(err);
        rec.status = EventStatus::Received;
        rec.updated_at = Utc::now();
        if let Some(n) = self.notifiers.read().await.get(id) {
            n.notify_waiters();
        }
        Ok(())
    }

    /// Wait until the named event reaches `desired` status or the timeout elapses.
    pub async fn wait_for_status(&self, id: &str, desired: EventStatus, timeout: std::time::Duration) -> bool {
        use tokio::time::{timeout as ttimeout, Instant};
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(rec) = self.get(id).await {
                if rec.status == desired {
                    return true;
                }
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let remaining = deadline - now;
            // await the per-event notifier if present, otherwise small sleep
            if let Some(n) = self.notifiers.read().await.get(id) {
                let notified = n.notified();
                let _ = ttimeout(remaining, notified).await;
            } else {
                // fallback to a short sleep
                let _ = ttimeout(remaining, tokio::time::sleep(std::time::Duration::from_millis(50))).await;
            }
        }
    }
}
