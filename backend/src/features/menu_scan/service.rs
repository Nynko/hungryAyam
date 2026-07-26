use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use base64::Engine;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::api_errors::ApiError;
use super::dto::{MenuScanJobCreated, MenuScanJobStatus, MenuScanResponse};

// ── Constants ────────────────────────────────────────────────────

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Default model, used when the `ANTHROPIC_MODEL` env var is unset.
/// Anthropic periodically retires old model snapshots (see
/// https://platform.claude.com/docs/en/about-claude/models/migration-guide),
/// which surfaces as a 404 from the API. Overriding `ANTHROPIC_MODEL` lets
/// ops swap models without a code change/rebuild when that happens.
const DEFAULT_MODEL: &str = "claude-sonnet-5";
const MAX_TOKENS: u32 = 16384;

const GLOBAL_DAILY_LIMIT: u32 = 20;
const PER_USER_DAILY_LIMIT: u32 = 5;

/// Maximum number of images to download from a webpage.
const MAX_URL_IMAGES: usize = 20;
/// Maximum size per downloaded image (5 MB).
const MAX_URL_IMAGE_BYTES: usize = 5 * 1024 * 1024;
/// Maximum number of paginated pages to follow.
const MAX_PAGES: u32 = 10;
/// Maximum number of detail pages (individual products) to fetch.
const MAX_DETAIL_PAGES: usize = 50;
/// Maximum total HTTP requests per scan.
const MAX_TOTAL_REQUESTS: usize = 60;

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
    model: String,
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
    db: PgPool,
    http_client: Client,
    anthropic_client: Client,
    api_key: String,
    model: String,
    rate_limit: Arc<Mutex<RateLimitState>>,
}

impl MenuScanService {
    pub fn new(db: PgPool) -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
        let model =
            std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        // Anthropic calls can take several minutes with many images.
        let anthropic_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build Anthropic HTTP client");

        Self {
            db,
            http_client,
            anthropic_client,
            api_key,
            model,
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
            model: self.model.clone(),
            max_tokens: MAX_TOKENS,
            system: system_prompt,
            messages: vec![AnthropicMessage {
                role: "user",
                content,
            }],
        };

        let response = self
            .anthropic_client
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
            if status == reqwest::StatusCode::NOT_FOUND && msg.contains("model") {
                tracing::error!(
                    "Anthropic API error (404): {} — model \"{}\" is unavailable, likely \
                     retired. Set ANTHROPIC_MODEL to a current model id (see \
                     https://platform.claude.com/docs/en/about-claude/models/overview) and \
                     restart the backend.",
                    msg,
                    self.model,
                );
            } else {
                tracing::error!("Anthropic API error ({}): {}", status, msg);
            }
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


    /// Scan a menu from a URL: fetch the page (following pagination and
    /// internal links to category/product detail pages), extract images,
    /// and send everything to AI.
    async fn execute_url_scan(&self, url: &str, extra_images: Vec<(String, Vec<u8>)>) -> Result<MenuScanResponse, ApiError> {
        let base_url = url::Url::parse(url).map_err(|e| {
            ApiError::BadRequest(format!("Invalid URL: {e}"))
        })?;

        let mut listing_html = String::new();
        let mut all_image_urls: Vec<String> = Vec::new();
        let mut seen_image_urls = std::collections::HashSet::new();
        let mut seen_pages = std::collections::HashSet::new();
        let mut detail_links: Vec<String> = Vec::new();
        let mut seen_detail_links = std::collections::HashSet::new();
        let mut total_requests = 0usize;

        // ── Phase 1: Fetch listing pages (follow pagination) ─────
        let mut current_url = url.to_string();
        let mut listing_pages = 0u32;

        loop {
            if listing_pages >= MAX_PAGES || total_requests >= MAX_TOTAL_REQUESTS {
                break;
            }
            if !seen_pages.insert(current_url.clone()) {
                break; // Already visited
            }

            let html = self.fetch_page(&current_url).await?;
            total_requests += 1;
            listing_pages += 1;

            // Collect images
            for img_url in extract_image_urls(&html, &base_url) {
                if seen_image_urls.insert(img_url.clone()) {
                    all_image_urls.push(img_url);
                }
            }

            // Collect internal links (potential product/category pages)
            for link in extract_internal_links(&html, &base_url) {
                if seen_detail_links.insert(link.clone()) {
                    detail_links.push(link);
                }
            }

            listing_html.push_str(&html);
            listing_html.push_str("\n\n<!-- === NEXT LISTING PAGE === -->\n\n");

            // Follow pagination
            match extract_next_page_url(&html, &base_url) {
                Some(next) if !seen_pages.contains(&next) => {
                    tracing::info!("Following pagination to: {next}");
                    current_url = next;
                }
                _ => break,
            }
        }

        tracing::info!(
            "Phase 1: {listing_pages} listing page(s), {} internal links found from {url}",
            detail_links.len()
        );

        // ── Phase 2: Fetch detail pages concurrently ────────────
        let links_to_fetch: Vec<_> = detail_links
            .into_iter()
            .filter(|l| !seen_pages.contains(l))
            .take(MAX_DETAIL_PAGES)
            .collect();

        let detail_futures: Vec<_> = links_to_fetch
            .iter()
            .map(|link| {
                let client = self.http_client.clone();
                let link = link.clone();
                async move {
                    let res = client
                        .get(&link)
                        .header("User-Agent", "Mozilla/5.0 (compatible; HungryAyam/1.0)")
                        .send()
                        .await;
                    match res {
                        Ok(r) if r.status().is_success() => {
                            r.text().await.ok().map(|html| (link, html))
                        }
                        _ => None,
                    }
                }
            })
            .collect();

        let detail_results = futures::future::join_all(detail_futures).await;

        let mut detail_html = String::new();
        let mut detail_fetched = 0usize;

        for result in detail_results.into_iter().flatten() {
            let (link, html) = result;
            detail_fetched += 1;

            for img_url in extract_image_urls(&html, &base_url) {
                if seen_image_urls.insert(img_url.clone()) {
                    all_image_urls.push(img_url);
                }
            }

            detail_html.push_str(&format!("\n<!-- === DETAIL PAGE: {link} === -->\n"));
            detail_html.push_str(&html);
        }

        tracing::info!(
            "Phase 2: fetched {detail_fetched} detail page(s), {} total images from {url}",
            all_image_urls.len()
        );

        // ── Phase 3: Download images concurrently ────────────────
        let urls_to_fetch: Vec<_> = all_image_urls.into_iter().take(MAX_URL_IMAGES).collect();

        let image_futures: Vec<_> = urls_to_fetch
            .iter()
            .map(|img_url| self.download_image(img_url))
            .collect();

        let image_results = futures::future::join_all(image_futures).await;

        let mut images: Vec<(String, Vec<u8>)> = Vec::new();
        for result in image_results {
            match result {
                Ok(Some(img)) => images.push(img),
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("Failed to download image: {e}");
                }
            }
        }

        tracing::info!(
            "Downloaded {} images from {url}",
            images.len()
        );

        // ── Phase 4: Build prompt and call AI ────────────────────
        // Start with user-uploaded images (they take priority), then URL-fetched
        let has_extra = !extra_images.is_empty();
        let mut content: Vec<ContentBlock> = extra_images
            .into_iter()
            .chain(images)
            .map(|(mime, bytes)| ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64",
                    media_type: mime,
                    data: base64::engine::general_purpose::STANDARD.encode(&bytes),
                },
            })
            .collect();

        // Combine listing + detail HTML, truncate if too large
        let mut combined_html = listing_html;
        combined_html.push_str("\n\n<!-- ======== DETAIL PAGES ======== -->\n\n");
        combined_html.push_str(&detail_html);

        let html_truncated = if combined_html.len() > 200_000 {
            format!("{}... [truncated]", &combined_html[..200_000])
        } else {
            combined_html
        };

        let extra_note = if has_extra {
            " The first images were uploaded directly by the user and may show prices or other details not visible on the webpage."
        } else {
            ""
        };

        content.push(ContentBlock::Text {
            text: format!(
                "Please extract the menu from this restaurant website.\n\
                 I fetched {listing_pages} listing page(s) and {detail_fetched} \
                 detail page(s) from: {url}{extra_note}\n\n\
                 The HTML includes both the listing/category pages and individual \
                 product detail pages. Use the detail pages to get full descriptions \
                 and image URLs for each item.\n\n\
                 HTML content:\n{html_truncated}"
            ),
        });

        self.call_anthropic(URL_SYSTEM_PROMPT, content).await
    }

    /// Run the combined scan (uploaded images + optional URL) and return the menu.
    async fn execute_combined_scan(
        &self,
        images: Vec<(String, Vec<u8>)>,
        url: Option<String>,
    ) -> Result<MenuScanResponse, ApiError> {
        match url {
            Some(url) => self.execute_url_scan(&url, images).await,
            None => {
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
        }
    }

    /// Create an async scan job from images and/or a URL, spawn it in the background.
    pub async fn create_combined_job(
        &self,
        images: Vec<(String, Vec<u8>)>,
        url: Option<String>,
        user_id: Uuid,
    ) -> Result<MenuScanJobCreated, ApiError> {
        self.check_preconditions(user_id)?;

        let row = sqlx::query(
            "INSERT INTO menu_scan_jobs (user_id, url) VALUES ($1, $2) RETURNING id"
        )
        .bind(user_id)
        .bind(url.as_deref())
        .fetch_one(&self.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create scan job: {e}");
            ApiError::Internal("Failed to create scan job.".into())
        })?;
        let job_id: Uuid = row.get("id");

        let service = self.clone();
        tokio::spawn(async move {
            sqlx::query(
                "UPDATE menu_scan_jobs SET status = 'processing', updated_at = now() WHERE id = $1"
            )
            .bind(job_id)
            .execute(&service.db)
            .await
            .ok();

            match service.execute_combined_scan(images, url).await {
                Ok(result) => {
                    let json = serde_json::to_value(&result).unwrap_or_default();
                    sqlx::query(
                        "UPDATE menu_scan_jobs SET status = 'completed', result = $2, updated_at = now() WHERE id = $1"
                    )
                    .bind(job_id)
                    .bind(sqlx::types::Json(json))
                    .execute(&service.db)
                    .await
                    .ok();
                }
                Err(e) => {
                    sqlx::query(
                        "UPDATE menu_scan_jobs SET status = 'failed', error = $2, updated_at = now() WHERE id = $1"
                    )
                    .bind(job_id)
                    .bind(e.to_string())
                    .execute(&service.db)
                    .await
                    .ok();
                }
            }
        });

        Ok(MenuScanJobCreated { job_id })
    }

    /// Poll the status of an async URL scan job.
    pub async fn get_job(&self, job_id: Uuid, user_id: Uuid) -> Result<MenuScanJobStatus, ApiError> {
        let row = sqlx::query(
            "SELECT id, status, result, error FROM menu_scan_jobs WHERE id = $1 AND user_id = $2"
        )
        .bind(job_id)
        .bind(user_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch scan job: {e}");
            ApiError::Internal("Failed to fetch scan job.".into())
        })?
        .ok_or(ApiError::NotFound)?;

        let status: String = row.get("status");
        let result_val: Option<serde_json::Value> = row.get("result");
        let result = if status == "completed" {
            result_val.and_then(|v| serde_json::from_value::<MenuScanResponse>(v).ok())
        } else {
            None
        };

        Ok(MenuScanJobStatus {
            job_id: row.get("id"),
            status,
            result,
            error: row.get("error"),
        })
    }

    /// Fetch a single page and return its HTML body.
    async fn fetch_page(&self, url: &str) -> Result<String, ApiError> {
        let response = self
            .http_client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; HungryAyam/1.0)")
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch URL {url}: {e}");
                ApiError::BadRequest(format!("Could not fetch the URL: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(ApiError::BadRequest(format!(
                "URL returned status {}",
                response.status()
            )));
        }

        response.text().await.map_err(|e| {
            tracing::error!("Failed to read page body from {url}: {e}");
            ApiError::BadRequest("Could not read the page content.".into())
        })
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

    // Also extract from srcset and data-src (lazy loading)
    for cap in regex_lite::Regex::new(r#"(?:data-src|data-lazy-src)\s*=\s*["']([^"']+)["']"#)
        .unwrap()
        .captures_iter(html)
    {
        if let Some(src) = cap.get(1) {
            let src_str = src.as_str();
            if src_str.starts_with("data:") || src_str.ends_with(".svg") {
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

// ── Internal link extraction ─────────────────────────────────────

/// Extract internal links from HTML that likely point to product or category pages.
/// Filters out pagination, admin, cart, and other non-content links.
fn extract_internal_links(html: &str, base_url: &url::Url) -> Vec<String> {
    let mut links = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let base_host = base_url.host_str().unwrap_or("");

    let link_re = regex_lite::Regex::new(
        r#"<a[^>]+href\s*=\s*["']([^"'#]+)["']"#
    ).unwrap();

    // Patterns that indicate non-content pages
    let skip_patterns = [
        "/cart", "/checkout", "/account", "/login", "/register",
        "/wp-admin", "/wp-login", "/wp-content", "/feed",
        "/page/", "?add-to-cart", "?remove_item", "/tag/",
        "/author/", "/comment", "/search", "/privacy", "/terms",
        "/contact", "/about", "/faq", "javascript:", "mailto:",
        ".css", ".js", ".xml", ".json", ".pdf",
    ];

    for cap in link_re.captures_iter(html) {
        if let Some(href) = cap.get(1) {
            let href_str = href.as_str().trim();

            // Skip empty or fragment-only
            if href_str.is_empty() {
                continue;
            }

            // Resolve to absolute
            let absolute = match base_url.join(href_str) {
                Ok(u) => u,
                Err(_) => continue,
            };

            // Must be same host
            if absolute.host_str() != Some(base_host) {
                continue;
            }

            let abs_str = absolute.to_string();

            // Skip known non-content patterns
            let lower = abs_str.to_lowercase();
            if skip_patterns.iter().any(|p| lower.contains(p)) {
                continue;
            }

            // Skip the base URL itself (we already have it)
            let normalized = abs_str.trim_end_matches('/');
            let base_normalized = base_url.as_str().trim_end_matches('/');
            if normalized == base_normalized {
                continue;
            }

            if seen.insert(abs_str.clone()) {
                links.push(abs_str);
            }
        }
    }

    links
}

// ── Pagination extraction ────────────────────────────────────────

/// Extract the "next page" URL from HTML pagination links.
/// Handles common patterns: WooCommerce, WordPress, generic `rel="next"`.
fn extract_next_page_url(html: &str, base_url: &url::Url) -> Option<String> {
    // 1. Look for <link rel="next" href="..."> or <a rel="next" href="...">
    let rel_next_re = regex_lite::Regex::new(
        r#"<(?:link|a)[^>]+rel\s*=\s*["']next["'][^>]+href\s*=\s*["']([^"']+)["']"#
    ).unwrap();
    if let Some(cap) = rel_next_re.captures(html) {
        if let Some(href) = cap.get(1) {
            if let Ok(u) = base_url.join(href.as_str()) {
                return Some(u.to_string());
            }
        }
    }

    // Also check href before rel (some sites put href first)
    let rel_next_re2 = regex_lite::Regex::new(
        r#"<(?:link|a)[^>]+href\s*=\s*["']([^"']+)["'][^>]+rel\s*=\s*["']next["']"#
    ).unwrap();
    if let Some(cap) = rel_next_re2.captures(html) {
        if let Some(href) = cap.get(1) {
            if let Ok(u) = base_url.join(href.as_str()) {
                return Some(u.to_string());
            }
        }
    }

    // 2. Look for WooCommerce-style pagination: <a class="next page-numbers" href="...">
    let woo_next_re = regex_lite::Regex::new(
        r#"<a[^>]+class\s*=\s*["'][^"']*next[^"']*["'][^>]+href\s*=\s*["']([^"']+)["']"#
    ).unwrap();
    if let Some(cap) = woo_next_re.captures(html) {
        if let Some(href) = cap.get(1) {
            if let Ok(u) = base_url.join(href.as_str()) {
                return Some(u.to_string());
            }
        }
    }

    // Also href before class
    let woo_next_re2 = regex_lite::Regex::new(
        r#"<a[^>]+href\s*=\s*["']([^"']+)["'][^>]+class\s*=\s*["'][^"']*next[^"']*["']"#
    ).unwrap();
    if let Some(cap) = woo_next_re2.captures(html) {
        if let Some(href) = cap.get(1) {
            if let Ok(u) = base_url.join(href.as_str()) {
                return Some(u.to_string());
            }
        }
    }

    None
}
