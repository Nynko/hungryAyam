use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{AppendHeaders, IntoResponse},
    routing::{get, post, put},
    Router,
};
use serde::Deserialize;
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    auth::middleware::{
        build_clear_session_cookie, build_session_cookie,
        build_site_access_cookie, build_clear_site_access_cookie,
        AdminUser, AuthUser,
    },
    errors::{api_errors::ApiError, json_extractor::ApiJson},
    state::AppState,
    types::{email::Email, name::Name, response::ApiResponse, role::UserRole},
};

// ═══════════════════════════════════════════════════════════════════
// Router
// ═══════════════════════════════════════════════════════════════════

/// Public auth routes (no authentication required).
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/site-access", post(verify_site_access))
        .route("/api/auth/site-access/:token", get(verify_site_access_token))
        .route("/api/auth/guest", post(create_guest))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
}

/// Admin-only auth routes (require Admin role).
pub fn admin_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/users/register", post(admin_register_user))
        .route("/api/admin/users/:id/upgrade", post(admin_upgrade_user))
        .route("/api/admin/users/:id/role", put(admin_change_role))
}

// ═══════════════════════════════════════════════════════════════════
// DTOs
// ═══════════════════════════════════════════════════════════════════

/// Request body for verifying the shared site password.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct SiteAccessRequest {
    pub code: String,
}

/// Response after successful site-access verification.
#[derive(Debug, serde::Serialize, TS)]
#[ts(export)]
pub struct SiteAccessResponse {
    pub granted: bool,
}

/// Request body for creating a guest (NameWithCookie) user.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateGuestRequest {
    pub name: Name,
}

/// Request body for logging in with email + password.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Request body for admin-registering a new password user.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct RegisterPasswordUserRequest {
    pub name: Name,
    pub email: Email,
    pub password: String,
    pub role: UserRole,
}

/// Request body for upgrading a guest user to a password user.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpgradeUserRequest {
    pub email: Email,
    pub password: String,
    pub role: UserRole,
}

/// Request body for changing a user's role.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct ChangeRoleRequest {
    pub role: UserRole,
}

/// Response containing user data and the session token.
///
/// The token is also set as an HttpOnly cookie, but is returned in the
/// body for clients that prefer `Authorization: Bearer` headers.
#[derive(Debug, serde::Serialize, TS)]
#[ts(export)]
pub struct AuthResponseDto {
    pub user: crate::features::user::domain::User,
    pub token: String,
}

// ═══════════════════════════════════════════════════════════════════
// Site access handlers
// ═══════════════════════════════════════════════════════════════════

/// `POST /api/auth/site-access`
///
/// Verify the shared site password. The plaintext code is hashed with
/// SHA-256 and compared against the stored hash. On success, sets the
/// `site_access` cookie (whose value is the hash) so subsequent requests
/// are recognised as having User-level access.
///
/// Without this cookie (and without a valid user session), a visitor can
/// only view statistics (implicit Viewer).
pub async fn verify_site_access(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<SiteAccessRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let stored_hash = state
        .setup_repository
        .get_access_hash()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::Unauthorized)?;

    // Hash the incoming plaintext code and compare
    let input_hash = crate::auth::password::sha256_hex(&request.code);

    if input_hash != stored_hash {
        return Err(ApiError::Unauthorized);
    }

    let cookie = build_site_access_cookie(&stored_hash, 365);
    let body = ApiJson(ApiResponse::success(SiteAccessResponse { granted: true }));

    Ok((StatusCode::OK, [(header::SET_COOKIE, cookie)], body))
}

/// `GET /api/auth/site-access/:hash`
///
/// Verify site access via a URL-based hash (shareable link).
///
/// The hash is the SHA-256 hex digest of the access code. The admin can
/// share a link like `https://site.com/access/{hash}`.
/// On success, sets the `site_access` cookie.
pub async fn verify_site_access_token(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let stored_hash = state
        .setup_repository
        .get_access_hash()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::Unauthorized)?;

    if hash != stored_hash {
        return Err(ApiError::Unauthorized);
    }

    let cookie = build_site_access_cookie(&stored_hash, 365);
    let body = ApiJson(ApiResponse::success(SiteAccessResponse { granted: true }));

    Ok((StatusCode::OK, [(header::SET_COOKIE, cookie)], body))
}

// ═══════════════════════════════════════════════════════════════════
// Public handlers
// ═══════════════════════════════════════════════════════════════════

/// `POST /api/auth/guest`
///
/// Create a new guest (NameWithCookie) user and issue a session.
///
/// Returns the user, a session token, and sets the `session_token` cookie.
pub async fn create_guest(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<CreateGuestRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let auth_response = state.auth_service.create_guest(request.name).await?;

    let cookie = build_session_cookie(&auth_response.token, 30);
    let body = ApiJson(ApiResponse::success(AuthResponseDto {
        user: auth_response.user,
        token: auth_response.token,
    }));

    Ok((
        StatusCode::CREATED,
        [(header::SET_COOKIE, cookie)],
        body,
    ))
}

/// `POST /api/auth/login`
///
/// Authenticate with email + password and issue a session.
///
/// Returns the user, a session token, and sets the `session_token` cookie.
pub async fn login(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let auth_response = state
        .auth_service
        .login(&request.email, &request.password)
        .await
        .map_err(|_| ApiError::Unauthorized)?;

    let cookie = build_session_cookie(&auth_response.token, 7);
    let body = ApiJson(ApiResponse::success(AuthResponseDto {
        user: auth_response.user,
        token: auth_response.token,
    }));

    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        body,
    ))
}

/// `POST /api/auth/logout`
///
/// Destroy the current session and clear session + site-access cookies.
///
/// Does not require authentication — if the token is invalid or missing,
/// cookies are still cleared (idempotent).
pub async fn logout() -> impl IntoResponse {
    let clear_session = build_clear_session_cookie();
    let clear_site = build_clear_site_access_cookie();
    (
        AppendHeaders([
            (header::SET_COOKIE, clear_session),
            (header::SET_COOKIE, clear_site),
        ]),
        ApiJson(ApiResponse::success(())),
    )
}

/// `GET /api/auth/me`
///
/// Returns the currently authenticated user.
///
/// Requires a valid session (returns 401 if not authenticated).
pub async fn me(AuthUser(user): AuthUser) -> Result<ApiJson<ApiResponse<crate::features::user::domain::User>>, ApiError> {
    Ok(ApiJson(ApiResponse::success(user)))
}

// ═══════════════════════════════════════════════════════════════════
// Admin handlers
// ═══════════════════════════════════════════════════════════════════

/// `POST /api/admin/users/register`
///
/// Create a new password-authenticated user (admin only).
///
/// The admin provides name, email, plaintext password, and role.
/// The password is hashed before storage.
pub async fn admin_register_user(
    AdminUser(_admin): AdminUser,
    State(state): State<AppState>,
    ApiJson(request): ApiJson<RegisterPasswordUserRequest>,
) -> Result<(StatusCode, ApiJson<ApiResponse<crate::features::user::domain::User>>), ApiError> {
    let user = state
        .auth_service
        .register_password_user(request.name, request.email, &request.password, request.role)
        .await?;

    Ok((StatusCode::CREATED, ApiJson(ApiResponse::success(user))))
}

/// `POST /api/admin/users/:id/upgrade`
///
/// Upgrade a guest (NameWithCookie) user to a password user (admin only).
///
/// Sets the email, password hash, and role. The user's ID is preserved
/// so their order history and other data remains linked.
/// All existing sessions for the user are invalidated.
pub async fn admin_upgrade_user(
    AdminUser(_admin): AdminUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    ApiJson(request): ApiJson<UpgradeUserRequest>,
) -> Result<ApiJson<ApiResponse<crate::features::user::domain::User>>, ApiError> {
    let user = state
        .auth_service
        .upgrade_to_password(user_id, request.email, &request.password, request.role)
        .await?;

    Ok(ApiJson(ApiResponse::success(user)))
}

/// `PUT /api/admin/users/:id/role`
///
/// Change a user's role (admin only).
///
/// Only password-authenticated users can have roles.
pub async fn admin_change_role(
    AdminUser(_admin): AdminUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    ApiJson(request): ApiJson<ChangeRoleRequest>,
) -> Result<ApiJson<ApiResponse<crate::features::user::domain::User>>, ApiError> {
    let user = state
        .auth_service
        .change_role(user_id, request.role)
        .await?;

    Ok(ApiJson(ApiResponse::success(user)))
}