//! Simplest possible keygen backend: an opaque random token with no
//! embedded structure. Validation is entirely a `Storage::get_license_by_key`
//! lookup done by the route handler -- this backend's own `validate` only
//! sanity-checks the shape of the string, since it holds no secret material
//! that could confirm anything cryptographically.

use async_trait::async_trait;
use rand::RngCore;

use thaumiel_core::registry::PluginContext;
use thaumiel_core::traits::{
    GenerateRequest, GeneratedKey, KeygenBackend, ValidateContext, Validation,
};
use thaumiel_core::Result;

const PREFIX: &str = "thm-lic-";

pub struct OpaqueTokenKeygen;

impl OpaqueTokenKeygen {
    pub fn new(_ctx: &PluginContext) -> Self {
        Self
    }
}

#[async_trait]
impl KeygenBackend for OpaqueTokenKeygen {
    fn id(&self) -> &'static str {
        "opaque"
    }

    fn description(&self) -> &'static str {
        "Opaque random token; validated purely by database lookup, no offline verification."
    }

    fn offline_verifiable(&self) -> bool {
        false
    }

    async fn generate(&self, _req: &GenerateRequest) -> Result<GeneratedKey> {
        let mut bytes = [0u8; 20];
        rand::thread_rng().fill_bytes(&mut bytes);
        let key = format!("{PREFIX}{}", hex::encode(bytes));
        Ok(GeneratedKey {
            key,
            backend_metadata: Default::default(),
        })
    }

    async fn validate(&self, key: &str, _ctx: &ValidateContext) -> Result<Validation> {
        if key.starts_with(PREFIX) && key.len() > PREFIX.len() {
            Ok(Validation::Valid)
        } else {
            Ok(Validation::Invalid {
                reason: "malformed opaque token".into(),
            })
        }
    }
}

thaumiel_core::register_keygen_backend!(OpaqueTokenKeygen::new);

#[cfg(test)]
mod tests {
    use super::*;
    use thaumiel_core::ids::{OrganizationId, ProductId};

    #[tokio::test]
    async fn generate_then_validate_round_trip() {
        let backend = OpaqueTokenKeygen;
        let req = GenerateRequest {
            org_id: OrganizationId::new(),
            product_id: ProductId::new(),
            seats: 1,
            expires_at: None,
            metadata: Default::default(),
        };
        let generated = backend.generate(&req).await.unwrap();
        assert!(generated.key.starts_with(PREFIX));

        let ctx = ValidateContext {
            org_id: req.org_id,
            product_id: req.product_id,
            backend_metadata: generated.backend_metadata,
        };
        assert_eq!(
            backend.validate(&generated.key, &ctx).await.unwrap(),
            Validation::Valid
        );
        assert!(matches!(
            backend.validate("garbage", &ctx).await.unwrap(),
            Validation::Invalid { .. }
        ));
    }
}
