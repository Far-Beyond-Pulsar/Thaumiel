//! Admin/dashboard session tokens: HMAC-SHA256 (HS256) signed JWTs carrying a
//! [`thaumiel_core::traits::Identity`].

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use thaumiel_core::ids::{OrganizationId, UserId};
use thaumiel_core::models::Role;
use thaumiel_core::traits::Identity;
use thaumiel_core::{Result, ThaumielError};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    org: String,
    email: String,
    role: Role,
    iat: i64,
    exp: i64,
}

/// Sign a session JWT for `identity`, valid for `ttl_secs` seconds from now.
pub fn issue_token(identity: &Identity, secret: &str, ttl_secs: u64) -> Result<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: identity.user_id.to_string(),
        org: identity.org_id.to_string(),
        email: identity.email.clone(),
        role: identity.role,
        iat: now,
        exp: now + ttl_secs as i64,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| ThaumielError::Crypto(format!("failed to sign session token: {e}")))
}

/// Verify a session JWT's signature and expiry, returning the [`Identity`] it
/// carries.
pub fn verify_token(token: &str, secret: &str) -> Result<Identity> {
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
        .map_err(|e| ThaumielError::Unauthenticated(format!("invalid session token: {e}")))?;
    let claims = data.claims;
    Ok(Identity {
        user_id: claims.sub.parse::<UserId>().map_err(|e| ThaumielError::Unauthenticated(e.to_string()))?,
        org_id: claims.org.parse::<OrganizationId>().map_err(|e| ThaumielError::Unauthenticated(e.to_string()))?,
        email: claims.email,
        role: claims.role,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use thaumiel_core::ids::{OrganizationId, UserId};

    #[test]
    fn issue_and_verify_roundtrip() {
        let identity = Identity {
            user_id: UserId::new(),
            org_id: OrganizationId::new(),
            email: "admin@example.com".into(),
            role: Role::Owner,
        };
        let token = issue_token(&identity, "test-secret", 3600).unwrap();
        let verified = verify_token(&token, "test-secret").unwrap();
        assert_eq!(verified.user_id, identity.user_id);
        assert_eq!(verified.email, identity.email);

        assert!(verify_token(&token, "wrong-secret").is_err());
    }
}
