use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use sha2::{Sha256, Digest};

/// Hash a plaintext password using Argon2id (OWASP recommended).
///
/// Returns the PHC-formatted hash string, e.g.:
/// `$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?
        .to_string();
    Ok(hash)
}

/// Verify a plaintext password against a stored Argon2 hash.
///
/// Returns `true` if the password matches, `false` otherwise.
/// Returns an error only if the stored hash is malformed.
pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| anyhow::anyhow!("Invalid password hash format: {}", e))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Compute the lowercase hex-encoded SHA-256 hash of a string.
///
/// Used for the site access code: the admin sets a memorable password,
/// and we store/compare its SHA-256 hash. The same hash appears in
/// shareable URLs and the `site_access` cookie.
///
/// This is NOT used for user passwords (those use Argon2).
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "my_secure_password_123!";
        let hash = hash_password(password).unwrap();

        // Hash should be a non-empty PHC string
        assert!(hash.starts_with("$argon2"));

        // Correct password verifies
        assert!(verify_password(password, &hash).unwrap());

        // Wrong password does not verify
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_different_hashes_for_same_password() {
        let password = "same_password";
        let hash1 = hash_password(password).unwrap();
        let hash2 = hash_password(password).unwrap();

        // Different salts produce different hashes
        assert_ne!(hash1, hash2);

        // But both verify against the same password
        assert!(verify_password(password, &hash1).unwrap());
        assert!(verify_password(password, &hash2).unwrap());
    }

    #[test]
    fn test_sha256_hex_deterministic() {
        let hash1 = sha256_hex("restaurant2024");
        let hash2 = sha256_hex("restaurant2024");
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // 256 bits = 64 hex chars
    }

    #[test]
    fn test_sha256_hex_different_inputs() {
        let hash1 = sha256_hex("password1");
        let hash2 = sha256_hex("password2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_sha256_hex_known_value() {
        // SHA-256("hello") is well-known
        let hash = sha256_hex("hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_verify_malformed_hash() {
        let result = verify_password("password", "not_a_valid_hash");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_password() {
        // Empty password should still hash and verify (the app layer should reject it, not the hasher)
        let hash = hash_password("").unwrap();
        assert!(verify_password("", &hash).unwrap());
        assert!(!verify_password("not_empty", &hash).unwrap());
    }
}