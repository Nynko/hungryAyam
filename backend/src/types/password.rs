use crate::validated_type;

/// Minimum password length enforced during validation.
pub const MIN_PASSWORD_LENGTH: usize = 8;

/// SHA-256 hash length in hex characters (64 chars = 256 bits).
pub const SHA256_HEX_LENGTH: usize = 64;

/// Conversion function for ClearPassword: String -> Result<ClearPassword, Error>
pub fn parse_clear_password(value: String) -> anyhow::Result<ClearPassword> {
    if value.len() < MIN_PASSWORD_LENGTH {
        anyhow::bail!(
            "Password must be at least {} characters",
            MIN_PASSWORD_LENGTH
        );
    }
    Ok(ClearPassword(value))
}

/// Conversion function for HashedPassword: String -> Result<HashedPassword, Error>
pub fn parse_hashed_password(value: String) -> anyhow::Result<HashedPassword> {
    if value.is_empty() {
        anyhow::bail!("Hashed password cannot be empty");
    }
    if value.len() != SHA256_HEX_LENGTH {
        anyhow::bail!(
            "Hashed password must be {} hex characters (SHA-256)",
            SHA256_HEX_LENGTH
        );
    }
    if !value.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("Hashed password must contain only hex characters");
    }
    Ok(HashedPassword(value))
}

validated_type!(
    /// A validated plaintext password (min 8 characters).
    ///
    /// This type represents user-provided password input before hashing.
    /// It validates length requirements but does NOT store hashed values.
    /// Use this for registration, login, and password change requests.
    pub ClearPassword(String) => String, parse_clear_password
);

validated_type!(
    /// A validated SHA-256 hashed password (64 hex characters).
    ///
    /// This type represents a password or access code that has been hashed
    /// with SHA-256. It validates that the value is exactly 64 hex characters.
    /// Used for storing access_hash in AppSetup and similar scenarios.
    pub HashedPassword(String) => String, parse_hashed_password
);

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // ClearPassword tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_valid_password() {
        let result = parse_clear_password("password123".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().inner(), "password123");
    }

    #[test]
    fn test_exactly_min_length() {
        let password = "a".repeat(MIN_PASSWORD_LENGTH);
        let result = parse_clear_password(password.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().inner(), &password);
    }

    #[test]
    fn test_too_short() {
        let password = "a".repeat(MIN_PASSWORD_LENGTH - 1);
        let result = parse_clear_password(password);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least 8 characters"));
    }

    #[test]
    fn test_empty_password() {
        let result = parse_clear_password("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_deref() {
        let password = parse_clear_password("mysecretpassword".to_string()).unwrap();
        // Can use string methods via Deref
        assert!(password.starts_with("my"));
        assert_eq!(password.len(), 16);
    }

    // ═══════════════════════════════════════════════════════════════
    // HashedPassword tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_valid_hashed_password() {
        // Valid SHA-256 hash (64 hex chars)
        let hash = "a".repeat(SHA256_HEX_LENGTH);
        let result = parse_hashed_password(hash.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().inner(), &hash);
    }

    #[test]
    fn test_hashed_password_mixed_case_hex() {
        // SHA-256 with mixed case hex digits
        let hash = "aAbBcCdDeEfF0123456789".to_string() + &"0".repeat(42);
        let result = parse_hashed_password(hash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hashed_password_empty_fails() {
        let result = parse_hashed_password("".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_hashed_password_wrong_length_fails() {
        // Too short
        let result = parse_hashed_password("abc123".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("64 hex characters"));

        // Too long
        let result = parse_hashed_password("a".repeat(65));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("64 hex characters"));
    }

    #[test]
    fn test_hashed_password_invalid_chars_fails() {
        // Contains non-hex characters
        let hash = "g".repeat(SHA256_HEX_LENGTH); // 'g' is not hex
        let result = parse_hashed_password(hash);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("hex characters"));
    }

    #[test]
    fn test_hashed_password_deref() {
        let hash = "f".repeat(SHA256_HEX_LENGTH);
        let hashed = parse_hashed_password(hash).unwrap();
        assert_eq!(hashed.len(), 64);
        assert!(hashed.starts_with("fff"));
    }
}