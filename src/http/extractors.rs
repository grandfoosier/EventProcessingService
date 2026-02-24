use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

#[derive(Clone)]
pub struct RequestId(pub String);

#[axum::async_trait]
impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(v) = parts.headers.get("x-request-id") {
            if let Ok(s) = v.to_str() {
                return Ok(RequestId(s.to_string()));
            }
        }
        Ok(RequestId(Uuid::new_v4().to_string()))
    }
}
