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

    /// Scan menu images and return structured menu data.
    ///
    /// `images` is a vec of `(mime_type, raw_bytes)`.
    pub async fn scan_menu_images(
        &self,
        images: Vec<(String, Vec<u8>)>,
        user_id: Uuid,
    ) -> Result<MenuScanResponse, ApiError> {
        if self.api_key.is_empty() {
            return Err(ApiError::Internal(
                "Menu scanning is not configured. Set ANTHROPIC_API_KEY to enable this feature.".into(),
            ));
        }

        // Check & increment rate limit
        {
            let mut rl = self.rate_limit.lock().unwrap();
            rl.check_and_increment(user_id)?;
        }

        // Build content blocks: images + text prompt
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

        let request_body = AnthropicRequest {
            model: MODEL,
            max_tokens: MAX_TOKENS,
            system: SYSTEM_PROMPT,
            messages: vec![AnthropicMessage {
                role: "user",
                content,
            }],
        };

        // Call Anthropic API
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

        // Parse response
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

        // Strip markdown fences if the model added them despite instructions
        let trimmed = text.trim();
        let json_text = if let Some(start) = trimmed.find('{') {
            let end = trimmed.rfind('}').unwrap_or(trimmed.len() - 1);
            &trimmed[start..=end]
        } else {
            trimmed
        };

        // Parse menu JSON
        let scan_result: MenuScanResponse = serde_json::from_str(json_text)
            .map_err(|e| {
                tracing::error!("Failed to parse menu JSON from AI: {e}\nRaw text: {json_text}");
                ApiError::Internal(format!(
                    "Could not parse menu from AI response: {e}"
                ))
            })?;

        Ok(scan_result)
    }
}
