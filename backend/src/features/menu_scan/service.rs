use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use base64::Engine;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::api_errors::ApiError;
use super::dto::MenuScanResponse;

// ── Constants ────────────────────────────────────────────────────

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MODEL: &str = "claude-sonnet-4-20250514";
const MAX_TOKENS: u32 = 16384;

const GLOBAL_DAILY_LIMIT: u32 = 20;
const PER_USER_DAILY_LIMIT: u32 = 5;

/// Maximum number of images to download from a webpage.
const MAX_URL_IMAGES: usize = 10;
/// Maximum size per downloaded image (5 MB).
const MAX_URL_IMAGE_BYTES: usize = 5 * 1024 * 1024;

const SYSTEM_PROMPT: &str = r#"You are a menu digitization assistant. You receive one or more photographs of a restaurant menu. Extract all menu items, organizing them into sections as they appear on the physical menu.

Return ONLY a JSON object (no markdown fences, no explanation, no text before or after) with this exact structure:
{
  "name": "<inferred menu name or 'Menu'>",
  "description": null,
  "sections": [
    {
      "name": "<section name>",
      "description": null,
      "position": <1-based sequential>,
      "items": [
        {
          "position": <1-based sequential within section>,
          "item": {
            "name": "<item name>",
            "description": "<brief description if visible, null otherwise>",
            "base_price_cents": <price in cents>,
            "tags": [{"name": "<tag>"}]
          }
        }
      ],
      "subsections": []
    }
  ]
}

Rules:
- Convert prices to cents (integer). "12.50" → 1250. "12,50" (European comma) → 1250. "12,500" (thousands separator) → 1250000.
- Infer tags from item names, descriptions, or symbols on the menu: "spicy", "very spicy", "vegetarian", "vegan", "gluten-free", "contains nuts", "seafood", "new", "popular", "chef's choice", "halal".
- If a section has subsections (e.g., "Appetizers > Hot / Cold"), use the subsections array.
- Keep the original language for names and descriptions — do NOT translate.
- If multiple images show the same menu, merge them into one structure; do not duplicate items.
- If a price is not visible for an item, set base_price_cents to 0.
- Position values must be sequential starting from 1 within each section/subsection.
- If no clear section structure exists, create a single section named "Menu"."#;

const URL_SYSTEM_PROMPT: &str = r#"You are a menu digitization assistant. You receive the HTML content of a restaurant menu webpage, possibly accompanied by images from the page. Extract all menu items, organizing them into sections as they appear.

Return ONLY a JSON object (no markdown fences, no explanation, no text before or after) with this exact structure:
{
  "name": "<inferred menu name or 'Menu'>",
  "description": null,
  "sections": [
    {
      "name": "<section name>",
      "description": null,
      "position": <1-based sequential>,
      "items": [
        {
          "position": <1-based sequential within section>,
          "item": {
            "name": "<item name>",
            "description": "<brief description if visible, null otherwise>",
            "base_price_cents": <price in cents>,
            "tags": [{"name": "<tag>"}],
            "image_url": "<absolute URL of the item image if visible on the page, null otherwise>"
          }
        }
      ],
      "subsections": []
    }
  ]
}

Rules:
- Convert prices to cents (integer). "12.50" → 1250. "12,50" (European comma) → 1250. "12,500" (thousands separator) → 1250000.
- Infer tags from item names, descriptions, or symbols on the menu: "spicy", "very spicy", "vegetarian", "vegan", "gluten-free", "contains nuts", "seafood", "new", "popular", "chef's choice", "halal".
- If a section has subsections (e.g., "Appetizers > Hot / Cold"), use the subsections array.
- Keep the original language for names and descriptions — do NOT translate.
- If a price is not visible for an item, set base_price_cents to 0.
- Position values must be sequential starting from 1 within each section/subsection.
- If no clear section structure exists, create a single section named "Menu".
- For image_url: return the absolute URL of item images found on the page. Only include images that clearly belong to a specific menu item, not decorative or background images. Return null if no image is found for an item.
- Extract all menu content from the page, including items that may be loaded in tabs, accordions, or hidden sections."#;

// ── Rate limiting ────────────────────────────────────────────────

struct RateLimitState {
    global_count: u32,
    global_reset_date: chrono::NaiveDate,
    per_user: HashMap<Uuid, (u32, chrono::NaiveDate)>,
}

impl RateLimitState {
    fn new() -> Self {
        Self {
            global_count: 0,
            global_reset_date: Utc::now().date_naive(),
            per_user: HashMap::new(),
        }
    }

    fn check_and_increment(&mut self, user_id: Uuid) -> Result<(), ApiError> {
        let today = Utc::now().date_naive();

        if self.global_reset_date != today {
            self.global_count = 0;
            self.global_reset_date = today;
        }

        let (user_count, user_date) = self
            .per_user
            .entry(user_id)
            .or_insert((0, today));
        if *user_date != today {
            *user_count = 0;
            *user_date = today;
        }

        if self.global_count >= GLOBAL_DAILY_LIMIT {
            return Err(ApiError::BadRequest(
                "Daily menu scan limit reached. Please try again tomorrow.".into(),
            ));
        }
        if *user_count >= PER_USER_DAILY_LIMIT {
            return Err(ApiError::BadRequest(
                "You have reached your daily menu scan limit (5 per day). Please try again tomorrow or create your menu manually.".into(),
            ));
        }

        self.global_count += 1;
        *user_count += 1;

        Ok(())
    }
}

// ── Anthropic API types ──────────────────────────────────────────

#[derive(Serialize)]
struct AnthropicRequest {
    model: &'static str,
    max_tokens: u32,
    system: &'static str,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: &'static str,
    content: Vec<ContentBlock>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Serialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: &'static str,
    media_type: String,
    data: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct AnthropicError {
    error: AnthropicErrorDetail,
}

#[derive(Deserialize)]
struct AnthropicErrorDetail {
    message: String,
}

// ── Service ──────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MenuScanService {
    http_client: Client,
    api_key: String,
    rate_limit: Arc<Mutex<RateLimitState>>,
}

impl MenuScanService {
    pub fn new() -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(90))
            .build()
            .expect("failed to build HTTP client");

        Self {
            http_client,
            api_key,
            rate_limit: Arc::new(Mutex::new(RateLimitState::new())),
        }
    }

    /// Check preconditions common to all scan methods.
    fn check_preconditions(&self, user_id: Uuid) -> Result<(), ApiError> {
        if self.api_key.is_empty() {
            return Err(ApiError::Internal(
                "Menu scanning is not configured. Set ANTHROPIC_API_KEY to enable this feature.".into(),
            ));
        }
        let mut rl = self.rate_limit.lock().unwrap();
        rl.check_and_increment(user_id)
    }

    /// Send content blocks to Anthropic and parse the menu JSON response.
    async fn call_anthropic(
        &self,
        system_prompt: &'static str,
        content: Vec<ContentBlock>,
    ) -> Result<MenuScanResponse, ApiError> {
        let request_body = AnthropicRequest {
            model: MODEL,
            max_tokens: MAX_TOKENS,
            system: system_prompt,
            messages: vec![AnthropicMessage {
                role: "user",
                content,
            }],
        };

        let response = self
            .http_client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Anthropic API request failed: {e}");
                ApiError::Internal("Failed to contact AI service. Please try again.".into())
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|e| {
            tracing::error!("Failed to read Anthropic response body: {e}");
            ApiError::Internal("Failed to read AI service response.".into())
        })?;

        if !status.is_success() {
            let msg = serde_json::from_str::<AnthropicError>(&body)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("AI service returned status {status}"));
            tracing::error!("Anthropic API error ({}): {}", status, msg);
            return Err(ApiError::Internal(format!("AI service error: {msg}")));
        }

        let api_response: AnthropicResponse = serde_json::from_str(&body)
            .context("Failed to parse Anthropic response")
            .map_err(|e| {
                tracing::error!("{e}");
                ApiError::Internal("Failed to parse AI service response.".into())
            })?;

        let text = api_response
            .content
            .into_iter()
            .find_map(|block| match block {
                AnthropicContentBlock::Text { text } => Some(text),
                _ => None,
            })
            .ok_or_else(|| {
                ApiError::Internal("AI returned no text content.".into())
            })?;

        // Extract JSON from response (handles markdown fences or surrounding text)
        let trimmed = text.trim();
        let json_text = if let Some(start) = trimmed.find('{') {
            let end = trimmed.rfind('}').unwrap_or(trimmed.len() - 1);
            &trimmed[start..=end]
        } else {
            trimmed
        };

        let scan_result: MenuScanResponse = serde_json::from_str(json_text)
            .map_err(|e| {
                tracing::error!("Failed to parse menu JSON from AI: {e}\nRaw text: {json_text}");
                ApiError::Internal(format!(
                    "Could not parse menu from AI response: {e}"
                ))
            })?;

        Ok(scan_result)
    }

    /// Scan menu images and return structured menu data.
    pub async fn scan_menu_images(
        &self,
        images: Vec<(String, Vec<u8>)>,
        user_id: Uuid,
    ) -> Result<MenuScanResponse, ApiError> {
        self.check_preconditions(user_id)?;

        let mut content: Vec<ContentBlock> = images
            .into_iter()
            .map(|(mime, bytes)| ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64",
                    media_type: mime,
                    data: base64::engine::general_purpose::STANDARD.encode(&bytes),
                },
            })
            .collect();

        content.push(ContentBlock::Text {
            text: "Please extract the menu from these images.".into(),
        });

        self.call_anthropic(SYSTEM_PROMPT, content).await
    }

    /// Scan a menu from a URL: fetch the page, extract images, and send to AI.
    pub async fn scan_menu_url(
        &self,
        url: &str,
        user_id: Uuid,
    ) -> Result<MenuScanResponse, ApiError> {
        self.check_preconditions(user_id)?;

        // Fetch the page HTML
        let page_response = self
            .http_client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; HungryAyam/1.0)")
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch URL {url}: {e}");
                ApiError::BadRequest(format!("Could not fetch the URL: {e}"))
            })?;

        if !page_response.status().is_success() {
            return Err(ApiError::BadRequest(format!(
                "URL returned status {}",
                page_response.status()
            )));
        }

        let html = page_response.text().await.map_err(|e| {
            tracing::error!("Failed to read page body from {url}: {e}");
            ApiError::BadRequest("Could not read the page content.".into())
        })?;

        // Extract image URLs from HTML
        let base_url = url::Url::parse(url).map_err(|e| {
            ApiError::BadRequest(format!("Invalid URL: {e}"))
        })?;
        let image_urls = extract_image_urls(&html, &base_url);

        tracing::info!(
            "Fetched {} chars of HTML and found {} images from {url}",
            html.len(),
            image_urls.len()
        );

        // Download images concurrently (limit to MAX_URL_IMAGES)
        let urls_to_fetch: Vec<_> = image_urls.into_iter().take(MAX_URL_IMAGES).collect();
        let mut images: Vec<(String, Vec<u8>)> = Vec::new();

        for img_url in &urls_to_fetch {
            match self.download_image(img_url).await {
                Ok(Some(img)) => images.push(img),
                Ok(None) => {} // Skipped (too large, wrong type, etc.)
                Err(e) => {
                    tracing::warn!("Failed to download image {img_url}: {e}");
                }
            }
        }

        tracing::info!("Successfully downloaded {} images from page", images.len());

        // Build content blocks: images first, then HTML text
        let mut content: Vec<ContentBlock> = images
            .into_iter()
            .map(|(mime, bytes)| ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64",
                    media_type: mime,
                    data: base64::engine::general_purpose::STANDARD.encode(&bytes),
                },
            })
            .collect();

        // Truncate HTML if too large (keep first 100KB which is plenty for menu content)
        let html_truncated = if html.len() > 100_000 {
            format!("{}... [truncated]", &html[..100_000])
        } else {
            html
        };

        content.push(ContentBlock::Text {
            text: format!(
                "Please extract the menu from this webpage. The page URL is: {url}\n\nHTML content:\n{html_truncated}"
            ),
        });

        self.call_anthropic(URL_SYSTEM_PROMPT, content).await
    }

    /// Download a single image, returning None if it should be skipped.
    async fn download_image(&self, url: &str) -> Result<Option<(String, Vec<u8>)>> {
        let response = self
            .http_client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; HungryAyam/1.0)")
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        // Check content type
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let mime = if content_type.contains("jpeg") || content_type.contains("jpg") {
            "image/jpeg"
        } else if content_type.contains("png") {
            "image/png"
        } else if content_type.contains("webp") {
            "image/webp"
        } else if content_type.contains("gif") {
            "image/gif"
        } else {
            // Skip non-image or unsupported formats (SVGs, ICOs, etc.)
            return Ok(None);
        };

        let bytes = response.bytes().await?;

        // Skip tiny images (likely icons/decorations) and oversized ones
        if bytes.len() < 5_000 || bytes.len() > MAX_URL_IMAGE_BYTES {
            return Ok(None);
        }

        Ok(Some((mime.to_string(), bytes.to_vec())))
    }
}

// ── HTML image extraction ────────────────────────────────────────

/// Extract image URLs from HTML, resolving relative URLs against the base.
fn extract_image_urls(html: &str, base_url: &url::Url) -> Vec<String> {
    let mut urls = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Simple regex-based extraction for src attributes in <img> tags
    // This handles most real-world HTML without needing a full parser
    for cap in regex_lite::Regex::new(r#"<img[^>]+src\s*=\s*["']([^"']+)["']"#)
        .unwrap()
        .captures_iter(html)
    {
        if let Some(src) = cap.get(1) {
            let src_str = src.as_str();
            // Skip data URIs, SVGs, and tracking pixels
            if src_str.starts_with("data:")
                || src_str.ends_with(".svg")
                || src_str.contains("pixel")
                || src_str.contains("tracking")
                || src_str.contains("spacer")
            {
                continue;
            }

            let absolute = match base_url.join(src_str) {
                Ok(u) => u.to_string(),
                Err(_) => continue,
            };

            if seen.insert(absolute.clone()) {
                urls.push(absolute);
            }
        }
    }

    urls
}
