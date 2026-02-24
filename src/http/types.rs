use crate::domain::event::{EventPayload, EventType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
pub struct EventIn {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct EventStatusOut {
    pub event_id: String,
    pub status: String,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub result: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<crate::domain::event::EventRecord> for EventStatusOut {
    fn from(rec: crate::domain::event::EventRecord) -> Self {
        Self {
            event_id: rec.event.event_id,
            status: format!("{:?}", rec.status),
            attempts: rec.attempts,
            last_error: rec.last_error,
            result: rec.result,
            created_at: rec.created_at,
            updated_at: rec.updated_at,
        }
    }
}

impl EventIn {
    pub fn into_domain(self) -> crate::domain::event::Event {
        let s = self.event_type.clone();
        let et = EventType::try_from(s.clone()).unwrap_or(EventType::Other(s));
        crate::domain::event::Event { event_id: self.event_id, event_type: et, occurred_at: self.occurred_at, payload: EventPayload(self.payload) }
    }
}
