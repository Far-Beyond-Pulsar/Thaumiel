//! Admin/dashboard session tokens: PASETO v4 local tokens carrying a
//! [`thaumiel_core::traits::Identity`].

use pasetors::claims::{Claims, ClaimsValidationRules};
use pasetors::keys::SymmetricKey;
use pasetors::token::Local;
use pasetors::token::UntrustedToken;
use pasetors::version4::V4;
use serde::{Deserialize, Serialize};

use thaumiel_core::ids::{OrganizationId, UserId};
use thaumiel_core::models::Role;
use thaumiel_core::traits::Identity;
use thaumiel_core::{Result, ThaumielError};

#[derive(Debug, Serialize, Deserialize)]
struct PasetoPayload {
    sub: String,
    org: String,
    email: String,
    role: Role,
    iat: i64,
    exp: i64,
}

/// Sign a PASETO session token for `identity`, valid for `ttl_secs` seconds from now.
pub fn issue_token(identity: &Identity, secret: &str, ttl_secs: u64) -> Result<String> {
    let now = chrono::Utc::now().timestamp();
    let payload = PasetoPayload {
        sub: identity.user_id.to_string(),
        org: identity.org_id.to_string(),
        email: identity.email.clone(),
        role: identity.role,
        iat: now,
        exp: now + ttl_secs as i64,
    };

    // We derive a valid 32-byte symmetric key from the provided secret.
    let key_bytes = blake3::hash(secret.as_bytes());
    let key_array: [u8; 32] = key_bytes.into();
    let key = SymmetricKey::<V4>::from(&key_array)
        .map_err(|e| ThaumielError::Crypto(format!("Invalid key: {:?}", e)))?;

    let mut claims =
        Claims::new().map_err(|_| ThaumielError::Crypto("failed to create claims".into()))?;
    let payload_json = serde_json::to_string(&payload)
        .map_err(|e| ThaumielError::Crypto(format!("failed to serialize payload: {}", e)))?;

    claims
        .add_additional("data", payload_json)
        .map_err(|_| ThaumielError::Crypto("failed to add claim".into()))?;

    pasetors::local::encrypt(&key, &claims, None, None)
        .map_err(|e| ThaumielError::Crypto(format!("failed to sign session token: {:?}", e)))
}

/// Verify a PASETO session token's signature and expiry, returning the [`Identity`] it
/// carries.
pub fn verify_token(token: &str, secret: &str) -> Result<Identity> {
    let key_bytes = blake3::hash(secret.as_bytes());
    let key_array: [u8; 32] = key_bytes.into();
    let key = SymmetricKey::<V4>::from(&key_array)
        .map_err(|e| ThaumielError::Crypto(format!("Invalid key: {:?}", e)))?;

    let untrusted = UntrustedToken::<Local, V4>::try_from(token)
        .map_err(|_| ThaumielError::Unauthenticated("invalid token format".into()))?;

    let validation_rules = ClaimsValidationRules::new();
    let trusted = pasetors::local::decrypt(&key, &untrusted, &validation_rules, None, None)
        .map_err(|e| ThaumielError::Unauthenticated(format!("invalid session token: {:?}", e)))?;

    let payload_str = trusted
        .payload_claims()
        .unwrap()
        .get_claim("data")
        .ok_or_else(|| ThaumielError::Unauthenticated("missing data claim".into()))?
        .as_str()
        .ok_or_else(|| ThaumielError::Unauthenticated("invalid data claim".into()))?;

    let payload: PasetoPayload = serde_json::from_str(payload_str)
        .map_err(|e| ThaumielError::Unauthenticated(format!("invalid payload: {}", e)))?;

    let now = chrono::Utc::now().timestamp();
    if now > payload.exp {
        return Err(ThaumielError::Unauthenticated(
            "session token expired".into(),
        ));
    }

    Ok(Identity {
        user_id: payload
            .sub
            .parse::<UserId>()
            .map_err(|e| ThaumielError::Unauthenticated(e.to_string()))?,
        org_id: payload
            .org
            .parse::<OrganizationId>()
            .map_err(|e| ThaumielError::Unauthenticated(e.to_string()))?,
        email: payload.email,
        role: payload.role,
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
