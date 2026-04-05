use axum::{
    Router,
    extract::{Multipart, State},
    http::StatusCode,
    routing::post,
};
use image::imageops::FilterType;
use serde::Serialize;
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    auth::middleware::EditorUser,
    errors::api_errors::ApiError,
    errors::json_extractor::ApiJson,
    state::AppState,
    types::response::ApiResponse,
};

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024; // 10 MB
const MAX_DIMENSION: u32 = 1200;
const WEBP_QUALITY: f32 = 82.0;

/// Allowed MIME types for uploaded images.
const ALLOWED_CONTENT_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/gif",
];

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct UploadResponse {
    pub url: String,
}

pub fn upload_routes() -> Router<AppState> {
    Router::new().route("/api/uploads", post(upload_image))
}

pub async fn upload_image(
    EditorUser(_user): EditorUser,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, ApiJson<ApiResponse<UploadResponse>>), ApiError> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::BadRequest(format!("multipart error: {e}"))
    })? {
        if field.name() == Some("file") {
            let ct = field
                .content_type()
                .map(|s| s.to_string())
                .unwrap_or_default();

            if !ALLOWED_CONTENT_TYPES.contains(&ct.as_str()) {
                return Err(ApiError::BadRequest(format!(
                    "unsupported file type: {ct}; allowed: jpeg, png, webp, gif"
                )));
            }

            let bytes = field.bytes().await.map_err(|e| {
                ApiError::BadRequest(format!("failed to read file: {e}"))
            })?;

            if bytes.len() > MAX_IMAGE_BYTES {
                return Err(ApiError::BadRequest(
                    "file exceeds 10 MB limit".to_string(),
                ));
            }

            content_type = Some(ct);
            file_bytes = Some(bytes.to_vec());
            break;
        }
    }

    let bytes = file_bytes.ok_or_else(|| ApiError::BadRequest("missing `file` field".to_string()))?;
    let _ct = content_type.unwrap_or_default();

    // Decode image
    let img = image::load_from_memory(&bytes)
        .map_err(|e| ApiError::BadRequest(format!("could not decode image: {e}")))?;

    // Downscale if either dimension exceeds MAX_DIMENSION
    let img = if img.width() > MAX_DIMENSION || img.height() > MAX_DIMENSION {
        img.resize(MAX_DIMENSION, MAX_DIMENSION, FilterType::Lanczos3)
    } else {
        img
    };

    // Encode to WebP with quality control via the `webp` crate.
    let webp_bytes = webp::Encoder::from_image(&img)
        .map_err(|e| ApiError::Internal(format!("failed to prepare WebP encoder: {e}")))?
        .encode(WEBP_QUALITY)
        .to_vec();

    // Write to disk
    let filename = format!("{}.webp", Uuid::new_v4());
    let file_path = state.upload_dir.join(&filename);

    tokio::fs::write(&file_path, &webp_bytes).await.map_err(|e| {
        ApiError::Internal(format!("failed to save file: {e}"))
    })?;

    let url = format!("/uploads/{filename}");
    Ok((
        StatusCode::CREATED,
        ApiJson(ApiResponse::success(UploadResponse { url })),
    ))
}
