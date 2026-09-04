//! Serves the embedded Next.js static export. Deliberately tries a few
//! candidate paths per request rather than assuming one exact output shape,
//! since exactly how `next build`'s static export lays out App Router routes
//! (`route.html` vs `route/index.html`) isn't a contract this crate wants to
//! depend on across Next.js versions.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

use crate::assets::Assets;

fn candidates(request_path: &str) -> Vec<String> {
    let trimmed = request_path.trim_start_matches('/');
    if trimmed.is_empty() {
        return vec!["index.html".to_string()];
    }
    vec![
        trimmed.to_string(),
        format!("{trimmed}.html"),
        format!("{trimmed}/index.html"),
    ]
}

fn asset_response(path: &str) -> Option<Response> {
    let file = Assets::get(path)?;
    let mime = file.metadata.mimetype();
    let mut response = Response::builder().status(StatusCode::OK).body(Body::from(file.data.into_owned())).unwrap();

    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_str(mime).unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")));

    // Next.js content-hashes everything under _next/static/, so it's safe --
    // and worth it for a dashboard someone reloads often -- to cache those
    // forever. Everything else (the HTML shells, the runtime config) should
    // always be revalidated, so a redeploy of the UI takes effect immediately.
    let cache_control = if path.starts_with("_next/static/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(cache_control));

    Some(response)
}

pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path();
    for candidate in candidates(path) {
        if let Some(response) = asset_response(&candidate) {
            return response;
        }
    }

    // A client-side route Next didn't prerender a file for, or a genuinely
    // missing asset -- either way, hand back 404.html if the app has one
    // (Next always emits one for `output: "export""), styled consistently
    // with the rest of the dashboard rather than a bare axum 404.
    if let Some(response) = asset_response("404.html") {
        let mut response = response;
        *response.status_mut() = StatusCode::NOT_FOUND;
        return response;
    }

    (StatusCode::NOT_FOUND, "not found").into_response()
}
