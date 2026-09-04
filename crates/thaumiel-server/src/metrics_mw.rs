//! Minimal request metrics: a total counter and a latency histogram, both
//! labeled by method/route-pattern/status, exported at `/metrics` in
//! Prometheus exposition format by `metrics-exporter-prometheus` (installed
//! in `main.rs`). The route *pattern* (`/v1/licenses/:id`, not the literal
//! path with a real id in it) is used as the label so cardinality stays
//! bounded regardless of traffic.

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;

pub async fn track(req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    let start = Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    metrics::counter!("http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status)
        .increment(1);
    metrics::histogram!("http_request_duration_seconds", "method" => method, "path" => path)
        .record(elapsed);

    response
}
