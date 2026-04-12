use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use uuid::Uuid;

use crate::{
    auth::middleware::EditorUser,
    errors::{api_errors::ApiError, json_extractor::ApiJson},
    state::AppState,
    types::response::ApiResponse,
};

use super::dto::{MenuScanJobCreated, MenuScanJobStatus};

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024; // 10 MB per image
const MAX_IMAGES: usize = 5;

const ALLOWED_CONTENT_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/webp",
];

pub fn menu_scan_routes() -> Router<AppState> {
    Router::new()
        .route("/api/menu-scan", post(scan_menu))
        .route("/api/menu-scan-jobs/:id", get(get_scan_job))
}

/// Accepts multipart with optional `images` file fields and an optional `url` text field.
/// At least one must be provided. Always returns a job ID for async polling.
async fn scan_menu(
    EditorUser(user): EditorUser,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, ApiJson<ApiResponse<MenuScanJobCreated>>), ApiError> {
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    let mut url: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::BadRequest(format!("multipart error: {e}"))
    })? {
        match field.name() {
            Some("images") => {
                if images.len() >= MAX_IMAGES {
                    return Err(ApiError::BadRequest(format!(
                        "Maximum {MAX_IMAGES} images allowed."
                    )));
                }

                let ct = field
                    .content_type()
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                if !ALLOWED_CONTENT_TYPES.contains(&ct.as_str()) {
                    return Err(ApiError::BadRequest(format!(
                        "Unsupported image type: {ct}. Allowed: JPEG, PNG, WebP."
                    )));
                }

                let bytes = field.bytes().await.map_err(|e| {
                    ApiError::BadRequest(format!("Failed to read image: {e}"))
                })?;

                if bytes.len() > MAX_IMAGE_BYTES {
                    return Err(ApiError::BadRequest(
                        "Each image must be under 10 MB.".into(),
                    ));
                }

                images.push((ct, bytes.to_vec()));
            }
            Some("url") => {
                let text = field.text().await.map_err(|e| {
                    ApiError::BadRequest(format!("Failed to read URL field: {e}"))
                })?;
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    url = Some(trimmed);
                }
            }
            _ => {}
        }
    }

    if images.is_empty() && url.is_none() {
        return Err(ApiError::BadRequest(
            "Provide at least one image or a URL.".into(),
        ));
    }

    if let Some(ref u) = url {
        if !u.starts_with("http://") && !u.starts_with("https://") {
            return Err(ApiError::BadRequest(
                "URL must start with http:// or https://".into(),
            ));
        }
    }

    let job = state
        .menu_scan_service
        .create_combined_job(images, url, user.id)
        .await?;

    Ok((StatusCode::ACCEPTED, ApiJson(ApiResponse::success(job))))
}

async fn get_scan_job(
    EditorUser(user): EditorUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, ApiJson<ApiResponse<MenuScanJobStatus>>), ApiError> {
    let status = state
        .menu_scan_service
        .get_job(id, user.id)
        .await?;

    Ok((StatusCode::OK, ApiJson(ApiResponse::success(status))))
}
