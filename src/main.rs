use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use serde_json::json;

use event_processing_service::telemetry::init_tracing;
use event_processing_service::Telemetry;
use event_processing_service::store::MemoryStore;
use event_processing_service::service::{IngestService, run_processor_pool};
use event_processing_service::domain::event::Event;
use event_processing_service::http::handlers::HttpState;
use event_processing_service::http::types::{EventIn, EventStatusOut};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing(None, false);
    let addr: SocketAddr = "127.0.0.1:3000".parse()?;
    info!(%addr, "starting background processor");

    let telemetry = Telemetry::new();
    let store = MemoryStore::new();
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);
    let ingest = IngestService::new(store.clone(), tx.clone(), telemetry.clone());

    // example handler: echo payload unless payload contains {"fail": true}
    let handler = |ev: Event| async move {
        if ev.payload.0.get("fail").and_then(|v| v.as_bool()).unwrap_or(false) {
            Err("simulated failure".to_string())
        } else {
            Ok(json!({"echo": ev.payload.0}).into())
        }
    };

    let shared_rx = std::sync::Arc::new(tokio::sync::Mutex::new(rx));
    run_processor_pool(store.clone(), shared_rx, tx.clone(),
    4, // worker count
    5, telemetry.clone(), handler);

    // build HTTP state
    let http_state = Arc::new(HttpState { ingest: ingest.clone(), store: store.clone(), telemetry: telemetry.clone() });

    // compile-time checks: ensure individual components are Send+Sync+'static.
    fn _assert_send_sync<T: Send + Sync + 'static>() {}
    _assert_send_sync::<HttpState>();
    _assert_send_sync::<event_processing_service::service::IngestService>();
    _assert_send_sync::<event_processing_service::store::MemoryStore>();
    _assert_send_sync::<event_processing_service::Telemetry>();

    // println!("listening on {}", listener.local_addr().unwrap());

        // run a small hyper service that forwards to our handlers without using axum
        let make_svc = make_service_fn(move |_conn| {
            let state = http_state.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                    let state = state.clone();
                    async move {
                        // routing
                        let path = req.uri().path().to_string();
                        match (req.method(), path.as_str()) {
                            // POST /events
                            (&Method::POST, "/events") => {
                                let bytes = match hyper::body::to_bytes(req.into_body()).await {
                                    Ok(b) => b,
                                    Err(e) => {
                                        tracing::error!(%e, "body read error");
                                        let mut resp = Response::new(Body::from("body read error"));
                                        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                                        return Ok::<_, Infallible>(resp);
                                    }
                                };
                                let evt_in: EventIn = match serde_json::from_slice(&bytes) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        tracing::error!(%e, "json parse error");
                                        let mut resp = Response::new(Body::from("invalid json"));
                                        *resp.status_mut() = StatusCode::BAD_REQUEST;
                                        return Ok::<_, Infallible>(resp);
                                    }
                                };
                                let ev = evt_in.into_domain();
                                let (rec, inserted) = state.ingest.ingest(ev).await;
                                let out = EventStatusOut::from(rec);
                                let body = serde_json::to_vec(&out).unwrap_or_default();
                                let mut resp = Response::new(Body::from(body));
                                *resp.status_mut() = if inserted { StatusCode::ACCEPTED } else { StatusCode::OK };
                                return Ok::<_, Infallible>(resp);
                            }
                            // GET /events/{id}
                            (&Method::GET, p) if p.starts_with("/events/") => {
                                let id = p.trim_start_matches("/events/");
                                match state.store.get(id).await {
                                    Ok(rec) => {
                                        let out = EventStatusOut::from(rec);
                                        let body = serde_json::to_vec(&out).unwrap_or_default();
                                        let mut resp = Response::new(Body::from(body));
                                        *resp.status_mut() = StatusCode::OK;
                                        return Ok::<_, Infallible>(resp);
                                    }
                                    Err(_) => {
                                        let mut resp = Response::new(Body::from("not found"));
                                        *resp.status_mut() = StatusCode::NOT_FOUND;
                                        return Ok::<_, Infallible>(resp);
                                    }
                                }
                            }
                            // healthz
                            (&Method::GET, "/healthz") => {
                                let q = state.telemetry.queue_depth.get();
                                let body = format!("ok - queue_depth={}", q);
                                return Ok::<_, Infallible>(Response::new(Body::from(body)));
                            }
                            // metrics
                            (&Method::GET, "/metrics") => {
                                let body = state.telemetry.gather();
                                return Ok::<_, Infallible>(Response::new(Body::from(body)));
                            }
                            _ => {
                                let mut resp = Response::new(Body::from("not found"));
                                *resp.status_mut() = StatusCode::NOT_FOUND;
                                return Ok::<_, Infallible>(resp);
                            }
                        }
                    }
                }))
            }
        });

        let server = Server::bind(&addr).serve(make_svc).with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.ok();
            info!("shutting down");
        });

        info!(%addr, "listening");
        server.await?;

    Ok(())
}
