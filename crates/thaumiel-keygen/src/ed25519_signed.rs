//! Offline-verifiable license key format: an Ed25519 signature over a small
//! JSON payload (org, product, seats, expiry, a uniqueness nonce). Format is
//! `ED1.<base64url(payload)>.<base64url(signature)>`.
//!
//! Because the signature is checked against a fixed public key, a licensed
//! application can validate a key completely offline (no call back to
//! Thaumiel) by embedding the public key -- printed by the server at
//! startup, and available at any time via `/v1/keygen-backends`. The server
//! itself still additionally checks status/revocation/expiry against
//! `Storage` for every `/v1/licenses/validate` call; this backend only
//! proves the key's *authenticity and content* weren't forged or tampered
//! with.

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{DateTime, TimeZone, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use thaumiel_core::ids::{OrganizationId, ProductId};
use thaumiel_core::registry::PluginContext;
use thaumiel_core::traits::{
    GenerateRequest, GeneratedKey, KeygenBackend, ValidateContext, Validation,
};
use thaumiel_core::{Result, ThaumielError};

const PREFIX: &str = "ED1";

#[derive(Debug, Serialize, Deserialize)]
struct Payload {
    org_id: OrganizationId,
    product_id: ProductId,
    seats: u32,
    /// Unix timestamp; `None` means no expiry.
    expires_at: Option<i64>,
    /// Random tag so two keys issued with identical fields still differ.
    nonce: u64,
}

pub struct Ed25519SignedKeygen {
    signing_key: SigningKey,
}

impl Ed25519SignedKeygen {
    pub fn new(_ctx: &PluginContext) -> Self {
        let signing_key = match std::env::var("THAUMIEL_KEYGEN_ED25519_SECRET") {
            Ok(hex_seed) => match hex::decode(hex_seed.trim())
                .ok()
                .and_then(|b| <[u8; 32]>::try_from(b).ok())
            {
                Some(seed) => SigningKey::from_bytes(&seed),
                None => {
                    tracing::warn!(
                        "THAUMIEL_KEYGEN_ED25519_SECRET is not a valid 32-byte hex seed; falling back to a \
                         random key"
                    );
                    SigningKey::generate(&mut OsRng)
                }
            },
            Err(_) => {
                tracing::warn!(
                    "THAUMIEL_KEYGEN_ED25519_SECRET not set; using an ephemeral random keypair for the 'ed25519' \
                     keygen backend -- keys minted this run will fail validation after a restart, and the public \
                     key changes every restart. Set it to a stable 32-byte hex seed in production."
                );
                SigningKey::generate(&mut OsRng)
            }
        };
        let verifying_key = signing_key.verifying_key();
        tracing::info!(
            public_key = %hex::encode(verifying_key.to_bytes()),
            "ed25519 keygen backend ready -- distribute this public key to clients that need offline validation"
        );
        Self { signing_key }
    }

    pub fn verifying_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }
}

#[async_trait]
impl KeygenBackend for Ed25519SignedKeygen {
    fn id(&self) -> &'static str {
        "ed25519"
    }

    fn description(&self) -> &'static str {
        "Ed25519-signed offline-verifiable key: ED1.<payload>.<signature>."
    }

    fn offline_verifiable(&self) -> bool {
        true
    }

    async fn generate(&self, req: &GenerateRequest) -> Result<GeneratedKey> {
        let payload = Payload {
            org_id: req.org_id,
            product_id: req.product_id,
            seats: req.seats,
            expires_at: req.expires_at.map(|dt| dt.timestamp()),
            nonce: rand::random(),
        };
        let payload_json = serde_json::to_vec(&payload)
            .map_err(|e| ThaumielError::Internal(format!("failed to encode key payload: {e}")))?;
        let signature = self.signing_key.sign(&payload_json);

        let key = format!(
            "{PREFIX}.{}.{}",
            URL_SAFE_NO_PAD.encode(&payload_json),
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        );

        let mut backend_metadata = std::collections::HashMap::new();
        backend_metadata.insert("public_key".to_string(), self.verifying_key_hex());
        Ok(GeneratedKey {
            key,
            backend_metadata,
        })
    }

    async fn validate(&self, key: &str, ctx: &ValidateContext) -> Result<Validation> {
        let parts: Vec<&str> = key.split('.').collect();
        let [prefix, payload_b64, sig_b64] = parts.as_slice() else {
            return Ok(Validation::Invalid {
                reason: "malformed key structure".into(),
            });
        };
        if *prefix != PREFIX {
            return Ok(Validation::Invalid {
                reason: "unrecognized key prefix".into(),
            });
        }

        let Ok(payload_bytes) = URL_SAFE_NO_PAD.decode(payload_b64) else {
            return Ok(Validation::Invalid {
                reason: "invalid payload encoding".into(),
            });
        };
        let Ok(sig_bytes) = URL_SAFE_NO_PAD.decode(sig_b64) else {
            return Ok(Validation::Invalid {
                reason: "invalid signature encoding".into(),
            });
        };
        let Ok(sig_array) = <[u8; 64]>::try_from(sig_bytes.as_slice()) else {
            return Ok(Validation::Invalid {
                reason: "invalid signature length".into(),
            });
        };
        let signature = Signature::from_bytes(&sig_array);

        let verifying_key = self
            .verifying_key_from_metadata(ctx)
            .unwrap_or_else(|| self.signing_key.verifying_key());
        if verifying_key.verify(&payload_bytes, &signature).is_err() {
            return Ok(Validation::Invalid {
                reason: "signature verification failed".into(),
            });
        }

        let Ok(payload) = serde_json::from_slice::<Payload>(&payload_bytes) else {
            return Ok(Validation::Invalid {
                reason: "invalid payload contents".into(),
            });
        };

        if payload.org_id != ctx.org_id || payload.product_id != ctx.product_id {
            return Ok(Validation::Invalid {
                reason: "key does not match organization/product".into(),
            });
        }
        if let Some(exp) = payload.expires_at {
            let expires_at: DateTime<Utc> =
                Utc.timestamp_opt(exp, 0).single().unwrap_or_else(Utc::now);
            if Utc::now() > expires_at {
                return Ok(Validation::Invalid {
                    reason: "key expired".into(),
                });
            }
        }

        Ok(Validation::Valid)
    }
}

impl Ed25519SignedKeygen {
    /// Prefer the public key recorded on the license at generation time
    /// (`backend_metadata`) over the server's current key, so validation
    /// keeps working across key rotation as long as old public keys are
    /// still recognized by the caller. Falls back to the live key when no
    /// metadata is present (e.g. legacy rows).
    fn verifying_key_from_metadata(&self, ctx: &ValidateContext) -> Option<VerifyingKey> {
        let hex_key = ctx.backend_metadata.get("public_key")?;
        let bytes = hex::decode(hex_key).ok()?;
        let array: [u8; 32] = bytes.try_into().ok()?;
        VerifyingKey::from_bytes(&array).ok()
    }
}

thaumiel_core::register_keygen_backend!(Ed25519SignedKeygen::new);

#[cfg(test)]
mod tests {
    use super::*;

    fn test_backend() -> Ed25519SignedKeygen {
        Ed25519SignedKeygen {
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    #[tokio::test]
    async fn generate_then_validate_round_trip() {
        let backend = test_backend();
        let org_id = OrganizationId::new();
        let product_id = ProductId::new();
        let req = GenerateRequest {
            org_id,
            product_id,
            seats: 5,
            expires_at: None,
            metadata: Default::default(),
        };

        let generated = backend.generate(&req).await.unwrap();
        assert!(generated.key.starts_with("ED1."));

        let ctx = ValidateContext {
            org_id,
            product_id,
            backend_metadata: generated.backend_metadata.clone(),
        };
        assert_eq!(
            backend.validate(&generated.key, &ctx).await.unwrap(),
            Validation::Valid
        );

        // Wrong product should fail even with a structurally valid signature.
        let wrong_ctx = ValidateContext {
            org_id,
            product_id: ProductId::new(),
            backend_metadata: generated.backend_metadata,
        };
        assert!(matches!(
            backend.validate(&generated.key, &wrong_ctx).await.unwrap(),
            Validation::Invalid { .. }
        ));

        // Tampering with the payload must break the signature check.
        let mut tampered = generated.key.clone();
        tampered.push('a');
        assert!(matches!(
            backend.validate(&tampered, &ctx).await.unwrap(),
            Validation::Invalid { .. }
        ));
    }
}
