#[cfg(test)]
mod tests {
    use axum::extract::FromRequestParts;
    use crate::http::extractors::RequestId;
    use axum::http::Request;
    use axum::http::header::HeaderName;
    use axum::http::HeaderValue;
    use std::sync::Arc;
    use crate::store::MemoryStore;
    use crate::telemetry::Telemetry;
    use crate::service::IngestService;
    use tokio::sync::mpsc;
    use crate::http::handlers::get_event;
    use axum::extract::State as AxState;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn request_id_extractor_generates_uuid_when_missing() {
        let req = Request::builder().uri("/").body(()).unwrap();
        let (mut parts, _body) = req.into_parts();
        let rid = RequestId::from_request_parts(&mut parts, &()).await.unwrap();
        assert!(!rid.0.is_empty());
    }

    #[tokio::test]
    async fn request_id_extractor_uses_header() {
        let req = Request::builder().uri("/").body(()).unwrap();
        let (mut parts, _body) = req.into_parts();
        parts.headers.insert(HeaderName::from_static("x-request-id"), HeaderValue::from_static("testid"));
        let rid = RequestId::from_request_parts(&mut parts, &()).await.unwrap();
        assert_eq!(rid.0, "testid");
    }

    #[tokio::test]
    async fn get_event_missing_returns_404() {
        // build minimal HttpState
        let store = MemoryStore::new();
        let telemetry = Telemetry::new();
        let (tx, _rx) = mpsc::channel::<String>(8);
        let ingest = IngestService::new(store.clone(), tx, telemetry.clone());
        let state = Arc::new(crate::http::handlers::HttpState { ingest, store: store.clone(), telemetry: telemetry.clone() });

        let resp = get_event(AxState(state.clone()), axum::extract::Path("nope".to_string())).await.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);
    }
}
