//! End-to-end smoke test driven entirely through the HTTP router (no real
//! network socket) against `InMemoryStorage` + `InMemoryCache`, exercising
//! the same code path `main.rs` wires up for real: register -> login ->
//! create product -> generate a license -> mint an API key -> validate the
//! license with it.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use thaumiel_config::AppConfig;
use thaumiel_core::registry::{AuthProviderRegistry, KeygenRegistry, PluginContext};
use thaumiel_server::{routes, AppState};

fn test_state() -> AppState {
    thaumiel_server::plugins::ensure_builtin_plugins_linked();
    let storage = Arc::new(thaumiel_storage::InMemoryStorage::new());
    let cache = Arc::new(thaumiel_cache::InMemoryCache::new());
    let ctx = PluginContext { storage: storage.clone(), cache: cache.clone() };
    AppState {
        config: Arc::new(AppConfig::default()),
        storage,
        cache,
        keygen: Arc::new(KeygenRegistry::from_inventory(&ctx)),
        auth_providers: Arc::new(AuthProviderRegistry::from_inventory(&ctx)),
    }
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn json_request(method: &str, uri: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri).header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn full_lifecycle() {
    let app = routes::build_router(test_state());

    // 1. Register a new org + owner user.
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/auth/register",
            None,
            json!({ "org_name": "Acme", "email": "owner@acme.test", "password": "hunter22222" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "register should succeed");
    let session = body_json(res).await;
    let token = session["token"].as_str().unwrap().to_string();

    // 2. Create a product using the default ("opaque") keygen backend.
    let res = app
        .clone()
        .oneshot(json_request("POST", "/v1/products", Some(&token), json!({ "name": "Widget Pro" })))
        .await
        .unwrap();
    let status = res.status();
    let product = body_json(res).await;
    assert_eq!(status, StatusCode::OK, "product creation should succeed: {product:?}");
    let product_id = product["id"].as_str().unwrap().to_string();
    assert_eq!(product["default_keygen_backend"], "opaque");

    // 3. Generate a license for that product.
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/licenses/generate",
            Some(&token),
            json!({ "product_id": product_id, "seats": 2 }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "license generation should succeed");
    let license = body_json(res).await;
    let license_key = license["key"].as_str().unwrap().to_string();
    assert!(license_key.starts_with("thm-lic-"));

    // 4. Mint an API key for the "validate" client flow.
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/api-keys",
            Some(&token),
            json!({ "name": "CI key", "scope": "validate_only" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "api key creation should succeed");
    let api_key = body_json(res).await;
    let api_key_plaintext = api_key["plaintext"].as_str().unwrap().to_string();

    // 5. Validate the license using the API key -- no admin session involved.
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/licenses/validate",
            Some(&api_key_plaintext),
            json!({ "key": license_key, "product_id": product_id, "machine_fingerprint": "machine-a" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "validation call should succeed");
    let validation = body_json(res).await;
    assert_eq!(validation["valid"], true, "license should validate: {validation:?}");
    assert_eq!(validation["seats_used"], 1);
    assert_eq!(validation["seats_total"], 2);

    // 6. A wrong key must fail closed, not error.
    let res = app
        .oneshot(json_request(
            "POST",
            "/v1/licenses/validate",
            Some(&api_key_plaintext),
            json!({ "key": "thm-lic-not-a-real-key", "product_id": product_id }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let validation = body_json(res).await;
    assert_eq!(validation["valid"], false);
}
