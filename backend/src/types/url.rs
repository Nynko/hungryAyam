use crate::validated_type;

/// Parses an `ImageSource` value. Accepts:
/// - an absolute `http`/`https` URL
/// - a local upload path (`/uploads/…`)
/// - a short emoji string (≤ 8 Unicode scalar values, no whitespace)
pub fn parse_image_source(value: String) -> anyhow::Result<ImageSource> {
    if value.starts_with("http://") || value.starts_with("https://") {
        http::Uri::try_from(value.as_str())
            .map_err(|e| anyhow::anyhow!("invalid URL: {e}"))?;
        return Ok(ImageSource(value));
    }
    if value.starts_with("/uploads/") {
        if value.contains("..") {
            anyhow::bail!("upload path must not contain '..'");
        }
        return Ok(ImageSource(value));
    }
    // Emoji / short display string.
    let char_count = value.chars().count();
    if char_count == 0 || char_count > 8 {
        anyhow::bail!(
            "image_url must be an http/https URL, a /uploads/ path, or an emoji (≤ 8 chars)"
        );
    }
    if value.chars().any(|c| c.is_ascii_whitespace()) {
        anyhow::bail!("emoji image value must not contain whitespace");
    }
    Ok(ImageSource(value))
}

validated_type!(
    /// An image source: an http/https URL, a local /uploads/ path, or an emoji.
    pub ImageSource(String) => String, parse_image_source
);
