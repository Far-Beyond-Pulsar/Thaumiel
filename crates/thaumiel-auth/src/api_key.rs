//! Machine/programmatic authentication for `/v1/licenses/*` and similar
//! endpoints: a `thm_live_<random>`-shaped secret, of which only a BLAKE3
//! hash and a short non-secret prefix are ever persisted
//! ([`thaumiel_core::models::ApiKey`]). The plaintext is shown to the caller
//! exactly once, at creation time.

use rand::RngCore;
use subtle::ConstantTimeEq;

pub struct GeneratedApiKey {
    /// Full secret. Show this to the caller once; never store it.
    pub plaintext: String,
    /// Short, non-secret identifier stored alongside the hash so keys can be
    /// looked up / shown in a UI without re-deriving anything from the hash.
    pub prefix: String,
    /// BLAKE3 hex digest of `plaintext`. Safe to store.
    pub hash: String,
}

fn blake3_hex(input: &str) -> String {
    let hash = blake3::hash(input.as_bytes());
    hash.to_hex().to_string()
}

/// Generate a new API key. `env_tag` is typically `"live"` or `"test"` and
/// becomes part of the visible prefix (e.g. `thm_live_9f2a1c3d...`).
pub fn generate_api_key(env_tag: &str) -> GeneratedApiKey {
    let mut random_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut random_bytes);
    let secret = hex::encode(random_bytes);

    let plaintext = format!("thm_{env_tag}_{secret}");
    let prefix = format!("thm_{env_tag}_{}", &secret[..8]);
    let hash = blake3_hex(&plaintext);

    GeneratedApiKey {
        plaintext,
        prefix,
        hash,
    }
}

/// Constant-time comparison of a freshly-hashed candidate secret against the
/// stored hash, to avoid leaking timing information about how many leading
/// hex characters matched.
pub fn verify_api_key(plaintext: &str, stored_hash: &str) -> bool {
    let candidate = blake3_hex(plaintext);
    if candidate.len() != stored_hash.len() {
        return false;
    }

    // Use subtle crate for guaranteed constant-time equality
    candidate.as_bytes().ct_eq(stored_hash.as_bytes()).into()
}

/// Extract the lookup prefix from a presented key, e.g. the `Authorization:
/// Bearer thm_live_...` header value, without needing to hash it first.
/// Returns `None` if the key is too short to plausibly contain a prefix.
pub fn prefix_of(plaintext: &str) -> Option<&str> {
    // "thm_<tag>_" + 8 hex chars; find the second-to-last underscore-delimited
    // segment boundary generically by locating the 8-char lookup suffix.
    let parts: Vec<&str> = plaintext.splitn(3, '_').collect();
    if parts.len() < 3 || parts[2].len() < 8 {
        return None;
    }
    let prefix_len = plaintext.len() - parts[2].len() + 8;
    plaintext.get(..prefix_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_verify_and_prefix_roundtrip() {
        let key = generate_api_key("live");
        assert!(key.plaintext.starts_with("thm_live_"));
        assert!(verify_api_key(&key.plaintext, &key.hash));
        assert!(!verify_api_key("thm_live_wrong", &key.hash));
        assert_eq!(prefix_of(&key.plaintext), Some(key.prefix.as_str()));
    }
}
