use crate::validated_type;
use email_address::EmailAddress;
use std::str::FromStr;

/// Conversion function for Email: String -> Result<Email, Error>
pub fn parse_email(value: String) -> anyhow::Result<Email> {
    let email = EmailAddress::from_str(&value)
        .map_err(|e| anyhow::anyhow!("Invalid email address: {}", e))?;
    Ok(Email(email))
}

/// Conversion function for encoding: &EmailAddress -> String
pub fn email_to_string(email: &EmailAddress) -> String {
    email.to_string()
}

validated_type!(
    /// A validated email address.
    pub Email(EmailAddress) => String, parse_email, email_to_string
);