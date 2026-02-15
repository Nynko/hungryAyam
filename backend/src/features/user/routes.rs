use axum::{
    routing::{delete, get, post},
    Router,
    extract::{State, Path},
    http::StatusCode,
};
use crate::types::email::parse_email;
use uuid::Uuid;

use crate::{
    auth::middleware::{AdminUser, AuthUser, SiteAccess},
    features::user::{
        dto::{CreateUserRequest, UpdateUserRequest},
        domain::User,
    },
    state::AppState,
    errors::{
        api_errors::ApiError,
        json_extractor::ApiJson,
    },
    types::response::ApiResponse,
};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/api/user", post(create_user))
        .route("/api/user/:id", get(get_user))
        .route("/api/users", get(list_users))
        .route("/api/update-user", post(update_user))
        .route("/api/user/:id", delete(delete_user))
        .route("/api/user/email/:email", get(get_user_by_email))
        .route("/api/user/name/:name", get(get_by_name))
}

/// Create a new guest (NameWithCookie) user (requires site access).
///
/// For Password user creation, use `POST /api/admin/users/register` instead.
pub async fn create_user(
    _site: SiteAccess,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<CreateUserRequest>,
) -> Result<(StatusCode, ApiJson<ApiResponse<User>>), ApiError> {
    let user = app_state.user_service.create_user(request).await?;
    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(user))))
}

/// Get a user by ID (requires site access)
pub async fn get_user(
    _site: SiteAccess,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<User>>, ApiError> {
    let user = app_state
        .user_service
        .get_user(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(user)))
}

/// Get a user by email (admin only)
pub async fn get_user_by_email(
    AdminUser(_admin): AdminUser,
    State(app_state): State<AppState>,
    Path(email): Path<String>,
) -> Result<ApiJson<ApiResponse<User>>, ApiError> {
    let parsed_email = parse_email(email)?;
    let user = app_state
        .user_service
        .get_user_by_email(&parsed_email)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(user)))
}

/// Get a user by name (requires site access)
pub async fn get_by_name(
    _site: SiteAccess,
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<ApiJson<ApiResponse<User>>, ApiError> {
    let user = app_state
        .user_service
        .get_by_name(&name)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(user)))
}

/// Get all users (admin only)
pub async fn list_users(
    AdminUser(_admin): AdminUser,
    State(app_state): State<AppState>,
) -> Result<ApiJson<ApiResponse<Vec<User>>>, ApiError> {
    let users = app_state.user_service.list_users().await?;
    Ok(ApiJson(ApiResponse::success(users)))
}

/// Update a user's name and/or email (ID provided in request body).
///
/// Does NOT update auth_method, password_hash, or role — those are
/// changed through dedicated admin endpoints.
/// Requires authenticated user.
pub async fn update_user(
    AuthUser(_user): AuthUser,
    State(app_state): State<AppState>,
    ApiJson(request): ApiJson<UpdateUserRequest>,
) -> Result<ApiJson<ApiResponse<User>>, ApiError> {
    let user = app_state
        .user_service
        .update_user(request)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(ApiJson(ApiResponse::success(user)))
}

/// Delete a user (admin only)
pub async fn delete_user(
    AdminUser(_admin): AdminUser,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    let deleted = app_state.user_service.delete_user(id).await?;
    if deleted {
        Ok(ApiJson(ApiResponse::success(())))
    } else {
        Err(ApiError::NotFound)
    }
}