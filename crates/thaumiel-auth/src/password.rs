//! Argon2id password hashing for internal-auth [`thaumiel_core::models::User`]
//! accounts.

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

use thaumiel_core::{Result, ThaumielError};

/// Hash a plaintext password into a PHC string (`$argon2id$v=19$...`) safe to
/// store in `User::password_hash`.
pub fn hash_password(plain: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ThaumielError::Crypto(format!("failed to hash password: {e}")))
}

/// Verify a plaintext password against a stored PHC hash. Returns `Ok(false)`
/// (not an error) for a merely-wrong password; only malformed hashes error.
pub fn verify_password(plain: &str, hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| ThaumielError::Crypto(format!("invalid stored hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash).unwrap());
        assert!(!verify_password("wrong password", &hash).unwrap());
    }
}
