use crate::validated_type;

/// Conversion function for UrlString: String -> Result<UrlString, Error>
pub fn parse_url(value: String) -> anyhow::Result<UrlString> {
    let url = url::Url::parse(&value)?;
    Ok(UrlString(url))
}

/// Conversion function for encoding: &Url -> String
pub fn url_to_string(url: &url::Url) -> String {
    url.to_string()
}

validated_type!(
    /// A validated URL string.
    pub UrlString(url::Url) => String, parse_url, url_to_string
);
