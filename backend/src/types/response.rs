use serde::Serialize;
use ts_rs::TS;

/// Standard API response wrapper for consistent response structure.
/// 
/// # Example Response (Success)
/// ```json
/// {
///     "success": true,
///     "data": [...]
/// }
/// ```
/// 
/// # Example Response (Error)
/// ```json
/// {
///     "success": false,
///     "error": "Not found"
/// }
/// ```
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Create a successful response with data
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Create an error response with a message
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Convenience type for list responses
pub type ApiListResponse<T> = ApiResponse<Vec<T>>;

/// Convenience type for optional item responses (e.g., get by ID)
pub type ApiOptionResponse<T> = ApiResponse<Option<T>>;