use crate::validated_type;

/// Conversion function for PriceCents: i32 -> Result<PriceCents, Error>
pub fn parse_price_cents(value: i32) -> anyhow::Result<PriceCents> {
    if value < 0 {
        anyhow::bail!("Price cannot be negative");
    }
    Ok(PriceCents(value))
}

validated_type!(
    /// A non-negative price in cents.
    /// Validates that the value is >= 0.
    pub PriceCents(i32) => i32, parse_price_cents
);
