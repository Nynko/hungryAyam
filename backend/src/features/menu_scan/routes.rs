use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    routing::post,
    Router,
};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::{
    auth::middleware::EditorUser,
    errors::{api_errors::ApiError, json_extractor::ApiJson},
    state::AppState,
    types::response::ApiResponse,
};

use super::dto::MenuScanResponse;

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
        .route("/api/menu-scan-url", post(scan_menu_url))
}

async fn scan_menu(
    EditorUser(user): EditorUser,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, ApiJson<ApiResponse<MenuScanResponse>>), ApiError> {
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::BadRequest(format!("multipart error: {e}"))
    })? {
        if field.name() != Some("images") {
            continue;
        }

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

    if images.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one image is required.".into(),
        ));
    }

    let result = state
        .menu_scan_service
        .scan_menu_images(images, user.id)
        .await?;

    Ok((StatusCode::OK, ApiJson(ApiResponse::success(result))))
}

#[derive(Deserialize)]
struct ScanUrlRequest {
    url: String,
}

async fn scan_menu_url(
    EditorUser(user): EditorUser,
    State(state): State<AppState>,
    ApiJson(body): ApiJson<ScanUrlRequest>,
) -> Result<(StatusCode, ApiJson<ApiResponse<MenuScanResponse>>), ApiError> {
    let url = body.url.trim().to_string();

    if url.is_empty() {
        return Err(ApiError::BadRequest("URL is required.".into()));
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(ApiError::BadRequest(
            "URL must start with http:// or https://".into(),
        ));
    }

    // Create a cancellation token that fires when the client disconnects
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Spawn the scan task so we can race it against client disconnect
    let service = state.menu_scan_service.clone();
    let scan_handle = tokio::spawn(async move {
        service.scan_menu_url(&url, user.id, cancel_clone).await
    });

    // Wait for either the scan to complete or the response to be dropped
    // (which happens on client disconnect / nginx timeout).
    // We use a drop guard to cancel when this handler returns early.
    let _cancel_guard = cancel.drop_guard();

    let result = scan_handle.await.map_err(|e| {
        tracing::error!("Scan task panicked: {e}");
        ApiError::Internal("Scan task failed unexpectedly.".into())
    })??;

    Ok((StatusCode::OK, ApiJson(ApiResponse::success(result))))
}
