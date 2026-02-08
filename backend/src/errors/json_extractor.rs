use axum::{
    async_trait,
    extract::{FromRequest, Request, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::types::response::ApiResponse;

/// Custom JSON extractor that returns errors in ApiResponse format
pub struct ApiJson<T>(pub T);

/// Custom rejection type that wraps JSON errors in ApiResponse format
pub struct ApiJsonRejection {
    message: String,
    status: StatusCode,
}

impl IntoResponse for ApiJsonRejection {
    fn into_response(self) -> Response {
        let body = Json(ApiResponse::<()>::error(&self.message));
        (self.status, body).into_response()
    }
}

impl From<JsonRejection> for ApiJsonRejection {
    fn from(rejection: JsonRejection) -> Self {
        let (status, message) = match rejection {
            JsonRejection::JsonDataError(err) => {
                // Deserialization error (validation, type mismatch, etc.)
                (StatusCode::BAD_REQUEST, err.body_text())
            }
            JsonRejection::JsonSyntaxError(err) => {
                // Malformed JSON
                (StatusCode::BAD_REQUEST, format!("Invalid JSON syntax: {}", err.body_text()))
            }
            JsonRejection::MissingJsonContentType(err) => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, err.body_text())
            }
            JsonRejection::BytesRejection(err) => {
                (StatusCode::BAD_REQUEST, err.body_text())
            }
            _ => {
                // Catch-all for any future rejection types
                (StatusCode::BAD_REQUEST, "Invalid request body".to_string())
            }
        };

        Self { message, status }
    }
}

#[async_trait]
impl<S, T> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiJsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(ApiJsonRejection::from)?;
        
        Ok(ApiJson(value))
    }
}

// Implement IntoResponse so ApiJson can be used as a response type
impl<T> IntoResponse for ApiJson<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        Json(self.0).into_response()
    }
}

// Allow destructuring like Json<T>
impl<T> std::ops::Deref for ApiJson<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for ApiJson<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}