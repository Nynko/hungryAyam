use crate::validated_type;

/// Conversion function for Name: String -> Result<Name, Error>
pub fn parse_name(value: String) -> anyhow::Result<Name> {
    if value.trim().is_empty() {
        anyhow::bail!("Name cannot be empty");
    }
    if value.len() > 100 {
        anyhow::bail!("Name cannot exceed 100 characters");
    }
    Ok(Name(value))
}

validated_type!(
    /// A validated name (non-empty, max 100 chars).
    pub Name(String) => String, parse_name
);
