use axum::extract::State;
use axum::Json;
use chrono::Utc;

use thaumiel_auth::{jwt, password};
use thaumiel_core::ids::{OrganizationId, UserId};
use thaumiel_core::models::{Organization, Role, User};
use thaumiel_core::traits::{Credentials, Identity};
use thaumiel_core::ThaumielError;

use crate::audit;
use crate::dto::{LoginRequest, OidcLoginRequest, RegisterRequest, SessionResponse};
use crate::error::ApiResult;
use crate::state::AppState;

/// Creates a brand-new organization plus its first user (role `owner`) in one
/// step, and logs them in. There is no separate "create organization" admin
/// route -- see `docs/ARCHITECTURE.md` for why (no superadmin/multi-org
/// concept in this build).
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> ApiResult<Json<SessionResponse>> {
    if req.password.len() < 8 {
        return Err(
            ThaumielError::InvalidInput("password must be at least 8 characters".into()).into(),
        );
    }

    let org = Organization {
        id: OrganizationId::new(),
        name: req.org_name,
        created_at: Utc::now(),
    };
    let org = state.storage.create_organization(org).await?;

    let user = User {
        id: UserId::new(),
        org_id: org.id,
        email: req.email.clone(),
        password_hash: Some(password::hash_password(&req.password)?),
        role: Role::Owner,
        created_at: Utc::now(),
    };
    let user = state.storage.create_user(user).await?;

    audit::record(
        &state,
        org.id,
        format!("user:{}", user.id),
        "organization.register",
        format!("organization:{}", org.id),
    )
    .await;

    let identity = Identity {
        user_id: user.id,
        org_id: org.id,
        email: user.email,
        role: user.role,
    };
    let token = jwt::issue_token(
        &identity,
        &state.config.auth.jwt_secret,
        state.config.auth.jwt_ttl_secs,
    )?;
    Ok(Json(SessionResponse { token, identity }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<SessionResponse>> {
    let provider = state.auth_providers.get(&state.config.auth.provider)?;
    let identity = provider
        .authenticate(Credentials::Password {
            org_id: req.org_id,
            email: req.email,
            password: req.password,
        })
        .await?;

    audit::record(
        &state,
        identity.org_id,
        format!("user:{}", identity.user_id),
        "auth.login",
        format!("user:{}", identity.user_id),
    )
    .await;

    let token = jwt::issue_token(
        &identity,
        &state.config.auth.jwt_secret,
        state.config.auth.jwt_ttl_secs,
    )?;
    Ok(Json(SessionResponse { token, identity }))
}

#[cfg(feature = "saml")]
mod saml_routes {
    use axum::extract::{Form, Query, State};
    use axum::response::{IntoResponse, Redirect, Response};
    use axum::Json;
    use serde::Deserialize;

    use thaumiel_auth::jwt;
    use thaumiel_core::ids::OrganizationId;

    use crate::audit;
    use crate::dto::SessionResponse;
    use crate::error::{ApiError, ApiResult};
    use crate::state::AppState;

    /// This server's own SAML SP metadata -- what an IdP administrator
    /// points at to configure the other side of the trust relationship.
    /// Public, no auth: SP metadata (entity id, ACS URL) isn't secret.
    pub async fn saml_metadata(State(state): State<AppState>) -> Response {
        match state.saml.metadata_xml().await {
            Ok(xml) => (
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/samlmetadata+xml",
                )],
                xml,
            )
                .into_response(),
            Err(e) => ApiError::from(e).into_response(),
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct SamlStartQuery {
        org_id: OrganizationId,
    }

    /// Redirects the browser to the IdP to begin an SP-initiated login for
    /// `?org_id=`. See `thaumiel_auth::saml`'s module doc comment for why
    /// this (not a JSON POST) is how a SAML login has to start.
    pub async fn saml_start(
        State(state): State<AppState>,
        Query(q): Query<SamlStartQuery>,
    ) -> ApiResult<Redirect> {
        let url = state.saml.login_redirect_url(q.org_id).await?;
        Ok(Redirect::temporary(&url))
    }

    #[derive(Debug, Deserialize)]
    pub struct SamlAcsForm {
        #[serde(rename = "SAMLResponse")]
        saml_response: String,
        #[serde(rename = "RelayState", default)]
        relay_state: String,
    }

    /// The IdP POSTs here after a successful login. Returns the session as
    /// JSON directly rather than redirecting into a UI -- there's no
    /// browser-facing callback page built for this yet (noted in
    /// docs/API.md); a real deployment would put one in front of this route.
    pub async fn saml_acs(
        State(state): State<AppState>,
        Form(form): Form<SamlAcsForm>,
    ) -> ApiResult<Json<SessionResponse>> {
        let identity = state
            .saml
            .handle_acs(&form.saml_response, &form.relay_state)
            .await?;

        audit::record(
            &state,
            identity.org_id,
            format!("user:{}", identity.user_id),
            "auth.login",
            format!("user:{}", identity.user_id),
        )
        .await;

        let token = jwt::issue_token(
            &identity,
            &state.config.auth.jwt_secret,
            state.config.auth.jwt_ttl_secs,
        )?;
        Ok(Json(SessionResponse { token, identity }))
    }
}

#[cfg(feature = "saml")]
pub use saml_routes::{saml_acs, saml_metadata, saml_start};

/// A separate route from `/v1/auth/login`, not an alternate branch of it --
/// see `thaumiel_auth::oidc`'s module doc comment for why. Always routes to
/// the `"oidc"` provider specifically, regardless of `auth.provider`, so a
/// deployment can offer password and OIDC login side by side.
pub async fn login_oidc(
    State(state): State<AppState>,
    Json(req): Json<OidcLoginRequest>,
) -> ApiResult<Json<SessionResponse>> {
    let provider = state.auth_providers.get("oidc")?;
    let identity = provider
        .authenticate(Credentials::OidcToken {
            org_id: req.org_id,
            id_token: req.id_token,
        })
        .await?;

    audit::record(
        &state,
        identity.org_id,
        format!("user:{}", identity.user_id),
        "auth.login",
        format!("user:{}", identity.user_id),
    )
    .await;

    let token = jwt::issue_token(
        &identity,
        &state.config.auth.jwt_secret,
        state.config.auth.jwt_ttl_secs,
    )?;
    Ok(Json(SessionResponse { token, identity }))
}
