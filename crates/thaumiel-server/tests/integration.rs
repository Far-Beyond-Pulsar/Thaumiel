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
        #[cfg(feature = "saml")]
        saml: Arc::new(thaumiel_auth::SamlAuthProvider::new(&ctx)),
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

/// Issues #7 and #9: a `license_manager`-scoped API key (not an admin JWT)
/// can generate a license and list/revoke its activations, and a
/// `validate_only` key is turned away from that same route.
#[tokio::test]
async fn license_manager_api_key_can_manage_licenses() {
    let app = routes::build_router(test_state());

    let session = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/auth/register",
                None,
                json!({ "org_name": "Acme", "email": "owner@acme.test", "password": "hunter22222" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let admin_token = session["token"].as_str().unwrap().to_string();

    let product = body_json(
        app.clone()
            .oneshot(json_request("POST", "/v1/products", Some(&admin_token), json!({ "name": "Widget Pro" })))
            .await
            .unwrap(),
    )
    .await;
    let product_id = product["id"].as_str().unwrap().to_string();

    let manager_key = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/api-keys",
                Some(&admin_token),
                json!({ "name": "ops bot", "scope": "license_manager" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let manager_key = manager_key["plaintext"].as_str().unwrap().to_string();

    let validate_only_key = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/api-keys",
                Some(&admin_token),
                json!({ "name": "client app", "scope": "validate_only" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let validate_only_key = validate_only_key["plaintext"].as_str().unwrap().to_string();

    // A validate_only key must not be able to generate a license.
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/licenses/generate",
            Some(&validate_only_key),
            json!({ "product_id": product_id, "seats": 1 }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "validate_only key must not manage licenses");

    // A license_manager key must be able to, with no admin session involved.
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/licenses/generate",
            Some(&manager_key),
            json!({ "product_id": product_id, "seats": 2 }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let license = body_json(res).await;
    let license_id = license["id"].as_str().unwrap().to_string();
    let license_key = license["key"].as_str().unwrap().to_string();

    // Activate two seats, then free one via the new endpoint (issue #7).
    for fingerprint in ["machine-a", "machine-b"] {
        let res = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/licenses/validate",
                Some(&validate_only_key),
                json!({ "key": license_key, "product_id": product_id, "machine_fingerprint": fingerprint }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    let activations = body_json(
        app.clone()
            .oneshot(json_request("GET", &format!("/v1/licenses/{license_id}/activations"), Some(&admin_token), json!({})))
            .await
            .unwrap(),
    )
    .await;
    let activations = activations.as_array().unwrap();
    assert_eq!(activations.len(), 2);
    let activation_id = activations[0]["id"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(json_request(
            "DELETE",
            &format!("/v1/licenses/{license_id}/activations/{activation_id}"),
            Some(&admin_token),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let activations = body_json(
        app.oneshot(json_request("GET", &format!("/v1/licenses/{license_id}/activations"), Some(&admin_token), json!({})))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(activations.as_array().unwrap().len(), 1, "one seat should have been freed");
}

/// Issue #10: an out-of-range `limit` is clamped rather than rejected or
/// left unbounded.
#[tokio::test]
async fn list_endpoints_respect_pagination_params() {
    let app = routes::build_router(test_state());

    let session = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/auth/register",
                None,
                json!({ "org_name": "Acme", "email": "owner@acme.test", "password": "hunter22222" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let token = session["token"].as_str().unwrap().to_string();

    for i in 0..3 {
        app.clone()
            .oneshot(json_request("POST", "/v1/products", Some(&token), json!({ "name": format!("Product {i}") })))
            .await
            .unwrap();
    }

    let page = body_json(
        app.oneshot(json_request("GET", "/v1/products?limit=2", Some(&token), json!({}))).await.unwrap(),
    )
    .await;
    assert_eq!(page.as_array().unwrap().len(), 2, "limit=2 should return exactly two products");
}

/// Issue #8: an owner can add a second user to their org, that user shows
/// up in the list, and the response never carries a password hash.
#[tokio::test]
async fn owner_can_invite_additional_users() {
    let app = routes::build_router(test_state());

    let session = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/auth/register",
                None,
                json!({ "org_name": "Acme", "email": "owner@acme.test", "password": "hunter22222" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let owner_token = session["token"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/users",
            Some(&owner_token),
            json!({ "email": "teammate@acme.test", "password": "hunter22222", "role": "member" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let created = body_json(res).await;
    assert_eq!(created["email"], "teammate@acme.test");
    assert!(created.get("password_hash").is_none(), "password_hash must never be serialized");

    let members_session = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/auth/login",
                None,
                json!({ "org_id": session["identity"]["org_id"], "email": "teammate@acme.test", "password": "hunter22222" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let member_token = members_session["token"].as_str().unwrap().to_string();

    // A member must not be able to invite further users.
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/users",
            Some(&member_token),
            json!({ "email": "another@acme.test", "password": "hunter22222", "role": "member" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let list = body_json(
        app.oneshot(json_request("GET", "/v1/users", Some(&owner_token), json!({}))).await.unwrap(),
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 2, "owner + the invited teammate");
}

/// Issue #6 (metering half): validate calls are counted per org per day,
/// and GET /v1/usage reflects both that history and current resource counts.
#[tokio::test]
async fn usage_summary_reflects_activity() {
    let app = routes::build_router(test_state());

    let session = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/auth/register",
                None,
                json!({ "org_name": "Acme", "email": "owner@acme.test", "password": "hunter22222" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let token = session["token"].as_str().unwrap().to_string();

    let product = body_json(
        app.clone()
            .oneshot(json_request("POST", "/v1/products", Some(&token), json!({ "name": "Widget Pro" })))
            .await
            .unwrap(),
    )
    .await;
    let product_id = product["id"].as_str().unwrap().to_string();

    let license = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/licenses/generate",
                Some(&token),
                json!({ "product_id": product_id, "seats": 1 }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let license_key = license["key"].as_str().unwrap().to_string();

    let api_key = body_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/api-keys",
                Some(&token),
                json!({ "name": "client", "scope": "validate_only" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let api_key = api_key["plaintext"].as_str().unwrap().to_string();

    // Three validate calls today -- valid or not shouldn't matter to the counter.
    for key in [license_key.as_str(), "thm-lic-does-not-exist", "thm-lic-also-missing"] {
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/licenses/validate",
                Some(&api_key),
                json!({ "key": key, "product_id": product_id }),
            ))
            .await
            .unwrap();
    }

    let summary = body_json(
        app.oneshot(json_request("GET", "/v1/usage", Some(&token), json!({}))).await.unwrap(),
    )
    .await;
    assert_eq!(summary["products"], 1);
    assert_eq!(summary["licenses_total"], 1);
    assert_eq!(summary["licenses_active"], 1);
    assert_eq!(summary["api_keys_active"], 1);
    let history = summary["validate_calls_last_14_days"].as_array().unwrap();
    assert_eq!(history.len(), 14, "always zero-filled to a fixed 14-day window");
    let today = history.last().unwrap();
    assert_eq!(today["count"], 3, "all three validate attempts, valid or not, should count");
}
