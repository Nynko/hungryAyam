use crate::validated_type;

/// Conversion function for Position: i32 -> Result<Position, Error>
pub fn parse_position(value: i32) -> anyhow::Result<Position> {
    if value < 0 {
        anyhow::bail!("Position cannot be negative");
    }
    Ok(Position(value))
}

validated_type!(
    /// A non-negative position.
    /// Validates that the value is >= 0.
    pub Position(i32) => i32, parse_position
);
