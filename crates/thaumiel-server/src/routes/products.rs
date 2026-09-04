use axum::extract::{Path, State};
use axum::Json;
use chrono::Utc;

use thaumiel_core::ids::ProductId;
use thaumiel_core::models::Product;
use thaumiel_core::traits::Pagination;
use thaumiel_core::ThaumielError;

use crate::audit;
use crate::dto::CreateProductRequest;
use crate::error::ApiResult;
use crate::extractors::AdminAuth;
use crate::state::AppState;

pub async fn create(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Json(req): Json<CreateProductRequest>,
) -> ApiResult<Json<Product>> {
    let default_keygen_backend = req.backend_or_default(&state);
    // Fail fast with a clear error if the requested backend isn't linked in,
    // rather than only discovering it the first time someone generates a key.
    state.keygen.get(&default_keygen_backend)?;

    let product = Product {
        id: ProductId::new(),
        org_id: identity.org_id,
        name: req.name,
        default_keygen_backend,
        created_at: Utc::now(),
    };
    let product = state.storage.create_product(product).await?;
    audit::record(&state, identity.org_id, format!("user:{}", identity.user_id), "product.create", format!("product:{}", product.id)).await;
    Ok(Json(product))
}

pub async fn list(State(state): State<AppState>, AdminAuth(identity): AdminAuth) -> ApiResult<Json<Vec<Product>>> {
    let products = state.storage.list_products(identity.org_id, Pagination::default()).await?;
    Ok(Json(products))
}

pub async fn get(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Path(id): Path<ProductId>,
) -> ApiResult<Json<Product>> {
    let product = state.storage.get_product(id).await?;
    if product.org_id != identity.org_id {
        return Err(ThaumielError::NotFound(format!("product '{id}'")).into());
    }
    Ok(Json(product))
}

impl CreateProductRequest {
    fn backend_or_default(&self, state: &AppState) -> String {
        self.default_keygen_backend.clone().unwrap_or_else(|| state.config.keygen.default_backend.clone())
    }
}
