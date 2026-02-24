use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventStatus {
    Received,
    Processing,
    Completed,
    Failed,
}

impl EventStatus {
    pub fn can_transition(self, next: EventStatus) -> bool {
        match (self, next) {
            (EventStatus::Received, EventStatus::Processing) => true,
            (EventStatus::Processing, EventStatus::Completed) => true,
            (EventStatus::Processing, EventStatus::Failed) => true,
            // allow requeue to Received if worker wants
            (EventStatus::Processing, EventStatus::Received) => true,
            _ => false,
        }
    }
}
