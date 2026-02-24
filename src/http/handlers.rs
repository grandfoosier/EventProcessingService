use crate::http::types::{EventIn, EventStatusOut};
use crate::service::IngestService;
use crate::store::MemoryStore;
use crate::Telemetry;
use axum::{extract::Path, extract::State, http::StatusCode, response::IntoResponse, Json};

pub struct HttpState {
    pub ingest: IngestService,
    pub store: MemoryStore,
    pub telemetry: Telemetry,
}

pub async fn post_events(State(state): State<std::sync::Arc<HttpState>>, Json(payload): Json<EventIn>) -> impl IntoResponse {
    let ev = payload.into_domain();
    let (rec, inserted) = state.ingest.ingest(ev).await;
    if inserted {
        (StatusCode::ACCEPTED, Json(EventStatusOut::from(rec))).into_response()
    } else {
        (StatusCode::OK, Json(EventStatusOut::from(rec))).into_response()
    }
}

pub async fn get_event(State(state): State<std::sync::Arc<HttpState>>, Path(id): Path<String>) -> impl IntoResponse {
    match state.store.get(&id).await {
        Ok(rec) => (StatusCode::OK, Json(EventStatusOut::from(rec))).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

pub async fn healthz(State(state): State<std::sync::Arc<HttpState>>) -> impl IntoResponse {
    let q = state.telemetry.queue_depth.get();
    (StatusCode::OK, format!("ok - queue_depth={}", q))
}

pub async fn metrics(State(state): State<std::sync::Arc<HttpState>>) -> impl IntoResponse {
    let body = state.telemetry.gather();
    (StatusCode::OK, body)
}
