use crate::domain::state::EventStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::convert::TryFrom;

/// Typed event type. Known variants can be listed here; unknown types are
/// preserved in `Other` so the system remains forward-compatible.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventType {
    #[serde(rename = "user.login_failed")]
    UserLoginFailed,
    Other(String),
}

impl From<EventType> for String {
    fn from(et: EventType) -> Self {
        match et {
            EventType::UserLoginFailed => "user.login_failed".to_string(),
            EventType::Other(s) => s,
        }
    }
}

impl TryFrom<String> for EventType {
    type Error = ();
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "user.login_failed" => Ok(EventType::UserLoginFailed),
            other => Ok(EventType::Other(other.to_string())),
        }
    }
}

/// Simple wrapper for the payload. Keep it extensible; for now we store raw
/// JSON so processors can interpret it according to `EventType`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventPayload(pub Value);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub event_id: String,
    pub event_type: EventType,
    pub occurred_at: DateTime<Utc>,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub event: Event,
    pub status: EventStatus,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub result: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl EventRecord {
    pub fn new(event: Event) -> Self {
        let now = Utc::now();
        Self {
            event,
            status: EventStatus::Received,
            attempts: 0,
            last_error: None,
            result: None,
            created_at: now,
            updated_at: now,
        }
    }
}
