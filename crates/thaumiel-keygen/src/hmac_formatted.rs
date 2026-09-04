//! Classic human-typable license key format: `HM-XXXX-XXXX-XXXX-XXXX-XXXX`.
//!
//! 16 random bytes plus a 4-byte HMAC-SHA256 checksum (truncated) of those
//! bytes are Base32-encoded and grouped for readability. `validate` recomputes
//! the checksum and compares it -- this proves the key was minted by *this*
//! server (knows the secret) but, unlike [`crate::ed25519_signed`], does not
//! cryptographically bind the key to a specific org/product; the route
//! handler enforces that separately via the stored `LicenseKey` row.

use async_trait::async_trait;
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

use thaumiel_core::registry::PluginContext;
use thaumiel_core::traits::{GenerateRequest, GeneratedKey, KeygenBackend, ValidateContext, Validation};
use thaumiel_core::Result;

type HmacSha256 = Hmac<Sha256>;

const PREFIX: &str = "HM-";
const RANDOM_LEN: usize = 16;
const CHECKSUM_LEN: usize = 4;

pub struct HmacFormattedKeygen {
    secret: Vec<u8>,
}

impl HmacFormattedKeygen {
    pub fn new(_ctx: &PluginContext) -> Self {
        let secret = match std::env::var("THAUMIEL_KEYGEN_HMAC_SECRET") {
            Ok(hex_secret) => hex::decode(hex_secret.trim()).unwrap_or_else(|_| {
                tracing::warn!("THAUMIEL_KEYGEN_HMAC_SECRET is not valid hex; falling back to a random secret");
                random_secret()
            }),
            Err(_) => {
                tracing::warn!(
                    "THAUMIEL_KEYGEN_HMAC_SECRET not set; using an ephemeral random secret for the 'hmac' \
                     keygen backend -- keys minted this run will fail validation after a restart. Set it to a \
                     stable hex string in production."
                );
                random_secret()
            }
        };
        Self { secret }
    }

    fn checksum(&self, random_part: &[u8]) -> [u8; CHECKSUM_LEN] {
        let mut mac = HmacSha256::new_from_slice(&self.secret).expect("HMAC accepts any key length");
        mac.update(random_part);
        let full = mac.finalize().into_bytes();
        let mut out = [0u8; CHECKSUM_LEN];
        out.copy_from_slice(&full[..CHECKSUM_LEN]);
        out
    }

    fn format_key(&self, random_part: &[u8], checksum: &[u8]) -> String {
        let mut payload = Vec::with_capacity(random_part.len() + checksum.len());
        payload.extend_from_slice(random_part);
        payload.extend_from_slice(checksum);
        let encoded = BASE32_NOPAD.encode(&payload);
        let grouped = encoded
            .as_bytes()
            .chunks(4)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join("-");
        format!("{PREFIX}{grouped}")
    }
}

fn random_secret() -> Vec<u8> {
    let mut bytes = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes
}

#[async_trait]
impl KeygenBackend for HmacFormattedKeygen {
    fn id(&self) -> &'static str {
        "hmac"
    }

    fn description(&self) -> &'static str {
        "Human-typable HM-XXXX-XXXX-... key with an embedded HMAC-SHA256 checksum."
    }

    fn offline_verifiable(&self) -> bool {
        false
    }

    async fn generate(&self, _req: &GenerateRequest) -> Result<GeneratedKey> {
        let mut random_part = [0u8; RANDOM_LEN];
        rand::thread_rng().fill_bytes(&mut random_part);
        let checksum = self.checksum(&random_part);
        let key = self.format_key(&random_part, &checksum);
        Ok(GeneratedKey { key, backend_metadata: Default::default() })
    }

    async fn validate(&self, key: &str, _ctx: &ValidateContext) -> Result<Validation> {
        let Some(body) = key.strip_prefix(PREFIX) else {
            return Ok(Validation::Invalid { reason: "missing key prefix".into() });
        };
        let stripped: String = body.chars().filter(|c| *c != '-').collect();
        let decoded = match BASE32_NOPAD.decode(stripped.to_uppercase().as_bytes()) {
            Ok(d) => d,
            Err(_) => return Ok(Validation::Invalid { reason: "invalid base32 encoding".into() }),
        };
        if decoded.len() != RANDOM_LEN + CHECKSUM_LEN {
            return Ok(Validation::Invalid { reason: "unexpected key length".into() });
        }
        let (random_part, checksum) = decoded.split_at(RANDOM_LEN);
        let expected = self.checksum(random_part);
        if expected.as_slice() == checksum {
            Ok(Validation::Valid)
        } else {
            Ok(Validation::Invalid { reason: "checksum mismatch".into() })
        }
    }
}

thaumiel_core::register_keygen_backend!(HmacFormattedKeygen::new);

#[cfg(test)]
mod tests {
    use super::*;
    use thaumiel_core::ids::{OrganizationId, ProductId};

    fn test_backend() -> HmacFormattedKeygen {
        HmacFormattedKeygen { secret: b"unit-test-secret".to_vec() }
    }

    #[tokio::test]
    async fn generate_then_validate_round_trip() {
        let backend = test_backend();
        let req = GenerateRequest {
            org_id: OrganizationId::new(),
            product_id: ProductId::new(),
            seats: 1,
            expires_at: None,
            metadata: Default::default(),
        };
        let generated = backend.generate(&req).await.unwrap();
        assert!(generated.key.starts_with(PREFIX));

        let ctx = ValidateContext { org_id: req.org_id, product_id: req.product_id, backend_metadata: Default::default() };
        assert_eq!(backend.validate(&generated.key, &ctx).await.unwrap(), Validation::Valid);

        let mut tampered = generated.key.clone();
        tampered.push('Z');
        assert!(matches!(backend.validate(&tampered, &ctx).await.unwrap(), Validation::Invalid { .. }));
    }
}
