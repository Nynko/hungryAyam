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
        AdminUser, AuthUser, SiteAccess,
        build_clear_session_cookie, build_clear_site_access_cookie, build_clear_site_access_hint_cookie,
        build_session_cookie, build_site_access_cookie, build_site_access_hint_cookie
    },
    errors::{api_errors::ApiError, json_extractor::ApiJson},
    state::AppState,
    types::{email::Email, name::Name, password::ClearPassword, response::ApiResponse, role::UserRole}, utils::password::sha256_hex,
};

// ═══════════════════════════════════════════════════════════════════
// Router
// ═══════════════════════════════════════════════════════════════════

/// Public auth routes (no authentication required).
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/site-access", get(check_site_access).post(verify_site_access))
        .route("/api/auth/site-access/:token", get(verify_site_access_token))
        .route("/api/auth/guest", post(create_guest))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
        .route("/api/auth/register", post(self_register))
        .route("/api/auth/verify-email", post(verify_email))
        .route("/api/auth/toggle-editor", post(toggle_editor))
        .route("/api/auth/profile/name", put(change_name))
        .route("/api/auth/editor-eligibility", get(check_editor_eligibility))
        .route("/api/admin/magic-link", get(get_magic_link_token))
}

/// Admin-only auth routes (require Admin role).
pub fn admin_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/users/register", post(admin_register_user))
        .route("/api/admin/users/:id/upgrade", post(admin_upgrade_user))
        .route("/api/admin/users/:id/role", put(admin_change_role))
        .route("/api/admin/settings/editor-domain", get(get_editor_domain).put(set_editor_domain))
        .route("/api/admin/settings/notification-email", get(get_notification_email).put(set_notification_email))
        .route("/api/admin/settings/test-notification-email", post(test_notification_email))
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
    pub password: ClearPassword,
    pub role: UserRole,
}

/// Request body for upgrading a guest user to a password user.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpgradeUserRequest {
    pub email: Email,
    pub password: ClearPassword,
    pub role: UserRole,
}

/// Request body for changing a user's role.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct ChangeRoleRequest {
    pub role: UserRole,
}

/// Request body for self-registering (guest → password account).
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct SelfRegisterRequest {
    pub email: Email,
    pub password: ClearPassword,
    /// Optional new display name. If omitted, the current name is kept.
    pub name: Option<Name>,
}

/// Request body for changing display name.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct ChangeNameRequest {
    pub name: Name,
}

/// Request body for setting the editor email domain.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct SetEditorDomainRequest {
    /// The email domain (e.g. "example.com"). `null` to clear.
    pub domain: Option<String>,
}

/// Response for editor eligibility check.
#[derive(Debug, serde::Serialize, TS)]
#[ts(export)]
pub struct EditorEligibilityResponse {
    /// Whether the user can toggle editor role.
    pub eligible: bool,
    /// Current role is Editor.
    pub is_editor: bool,
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
    /// `true` if this was an existing user (reconnected), `false` if newly created.
    /// Only relevant for guest creation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_user: Option<bool>,
}

// ═══════════════════════════════════════════════════════════════════
// Site access handlers
// ═══════════════════════════════════════════════════════════════════

/// `GET /api/auth/site-access`
///
/// Check whether the caller already has a valid `site_access` cookie.
/// Returns 200 if granted, 403 if not (via the `SiteAccess` extractor).
pub async fn check_site_access(_site: SiteAccess) -> ApiJson<ApiResponse<SiteAccessResponse>> {
    ApiJson(ApiResponse::success(SiteAccessResponse { granted: true }))
}

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
    let input_hash = sha256_hex(&request.code);

    if input_hash != *stored_hash {
        return Err(ApiError::InvalidPassword);
    }

    let cookie = build_site_access_cookie(&stored_hash, 365);
    let hint = build_site_access_hint_cookie(365);
    let body = ApiJson(ApiResponse::success(SiteAccessResponse { granted: true }));

    Ok((
        StatusCode::OK,
        AppendHeaders([
            (header::SET_COOKIE, cookie),
            (header::SET_COOKIE, hint),
        ]),
        body,
    ))
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

    if hash != *stored_hash {
        return Err(ApiError::Unauthorized);
    }

    let cookie = build_site_access_cookie(&stored_hash, 365);
    let hint = build_site_access_hint_cookie(365);
    let body = ApiJson(ApiResponse::success(SiteAccessResponse { granted: true }));

    Ok((
        StatusCode::OK,
        AppendHeaders([
            (header::SET_COOKIE, cookie),
            (header::SET_COOKIE, hint),
        ]),
        body,
    ))
}

// ═══════════════════════════════════════════════════════════════════
// Public handlers
// ═══════════════════════════════════════════════════════════════════

/// `POST /api/auth/guest`
///
/// Create a new guest (NameWithCookie) user and issue a session.
///
/// Requires site access (the `site_access` cookie must be set).
///
/// Returns the user, a session token, and sets the `session_token` cookie.
pub async fn create_guest(
    _site: SiteAccess,
    State(state): State<AppState>,
    ApiJson(request): ApiJson<CreateGuestRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (auth_response, existing) = state.auth_service.create_guest(request.name).await?;

    let cookie = build_session_cookie(&auth_response.token, 30);
    let body = ApiJson(ApiResponse::success(AuthResponseDto {
        user: auth_response.user,
        token: auth_response.token,
        existing_user: Some(existing),
    }));

    let status = if existing {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };

    Ok((
        status,
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
        existing_user: None,
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
    let clear_hint = build_clear_site_access_hint_cookie();
    (
        AppendHeaders([
            (header::SET_COOKIE, clear_session),
            (header::SET_COOKIE, clear_site),
            (header::SET_COOKIE, clear_hint),
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
// Self-service handlers (authenticated user)
// ═══════════════════════════════════════════════════════════════════

/// `POST /api/auth/register`
///
/// Upgrade the current guest account to a password-authenticated account.
///
/// All existing sessions are invalidated — the client must re-login
/// with the new credentials after a successful upgrade.
pub async fn self_register(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
    ApiJson(request): ApiJson<SelfRegisterRequest>,
) -> Result<ApiJson<ApiResponse<()>>, ApiError> {
    // Optionally rename first
    if let Some(new_name) = request.name {
        state.auth_service.change_name(user.id, new_name).await?;
    }

    state
        .auth_service
        .self_upgrade_to_password(user.id, request.email, &request.password)
        .await?;

    Ok(ApiJson(ApiResponse::success(())))
}

/// Request body for email verification.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct VerifyEmailRequest {
    pub token: String,
}

/// `POST /api/auth/verify-email`
///
/// Verify a user's email address using the token sent by email.
/// The token is single-use and expires after 24 hours.
pub async fn verify_email(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<VerifyEmailRequest>,
) -> Result<ApiJson<ApiResponse<crate::features::user::domain::User>>, ApiError> {
    let user = state
        .auth_service
        .user_repository_ref()
        .consume_verification_token(&request.token)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;

    Ok(ApiJson(ApiResponse::success(user)))
}

/// `POST /api/auth/toggle-editor`
///
/// Toggle between User and Editor roles. Only eligible registered users
/// whose email domain matches the configured `editor_email_domain`.
pub async fn toggle_editor(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
) -> Result<ApiJson<ApiResponse<crate::features::user::domain::User>>, ApiError> {
    let domain = state
        .setup_repository
        .get_editor_email_domain()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::BadRequest("Editor self-service is not configured".to_string()))?;

    let user = state.auth_service.toggle_editor(user.id, &domain).await?;

    Ok(ApiJson(ApiResponse::success(user)))
}

/// `PUT /api/auth/profile/name`
///
/// Change the current user's display name.
pub async fn change_name(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
    ApiJson(request): ApiJson<ChangeNameRequest>,
) -> Result<ApiJson<ApiResponse<crate::features::user::domain::User>>, ApiError> {
    let user = state.auth_service.change_name(user.id, request.name).await?;

    Ok(ApiJson(ApiResponse::success(user)))
}

/// `GET /api/auth/editor-eligibility`
///
/// Check if the current user is eligible to toggle Editor role.
/// Returns eligibility status and current editor state.
pub async fn check_editor_eligibility(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
) -> Result<ApiJson<ApiResponse<EditorEligibilityResponse>>, ApiError> {
    let is_editor = user.role.as_ref().map(|r| r.is_editor_or_above()).unwrap_or(false)
        && !user.role.as_ref().map(|r| r.is_admin()).unwrap_or(false);

    let domain = state
        .setup_repository
        .get_editor_email_domain()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let eligible = match (&domain, &user.email, &user.auth_method) {
        (Some(d), Some(email), &crate::types::auth::AuthMethod::Password) => {
            let domain_ok = d == "*" || {
                let user_domain = email.as_ref().domain();
                user_domain.eq_ignore_ascii_case(d)
            };
            domain_ok && !user.role.as_ref().map(|r| r.is_admin()).unwrap_or(false)
        }
        _ => false,
    };

    Ok(ApiJson(ApiResponse::success(EditorEligibilityResponse {
        eligible,
        is_editor,
    })))
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

/// `GET /api/admin/magic-link`
///
/// Returns the site-access token (SHA-256 hash) used to build shareable
/// magic links. Admin only.
pub async fn get_magic_link_token(
    AdminUser(_admin): AdminUser,
    State(state): State<AppState>,
) -> Result<ApiJson<ApiResponse<String>>, ApiError> {
    let hash = state
        .setup_repository
        .get_access_hash()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;

    Ok(ApiJson(ApiResponse::success(hash.as_ref().to_string())))
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

/// `GET /api/admin/settings/editor-domain`
///
/// Get the currently configured editor email domain.
pub async fn get_editor_domain(
    AdminUser(_admin): AdminUser,
    State(state): State<AppState>,
) -> Result<ApiJson<ApiResponse<Option<String>>>, ApiError> {
    let domain = state
        .setup_repository
        .get_editor_email_domain()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(ApiJson(ApiResponse::success(domain)))
}

/// `GET /api/admin/settings/notification-email`
///
/// Get the globally configured notification email address.
pub async fn get_notification_email(
    AdminUser(_admin): AdminUser,
    State(state): State<AppState>,
) -> Result<ApiJson<ApiResponse<Option<String>>>, ApiError> {
    let email = state
        .setup_repository
        .get_notification_email()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(ApiJson(ApiResponse::success(email)))
}

/// Request body for setting the notification email.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct SetNotificationEmailRequest {
    /// The email address to notify on session close. `null` to clear.
    pub email: Option<String>,
}

/// `PUT /api/admin/settings/notification-email`
///
/// Set or clear the global notification email address.
pub async fn set_notification_email(
    AdminUser(_admin): AdminUser,
    State(state): State<AppState>,
    ApiJson(request): ApiJson<SetNotificationEmailRequest>,
) -> Result<ApiJson<ApiResponse<Option<String>>>, ApiError> {
    let email = request.email.map(|e| e.trim().to_lowercase());

    state
        .setup_repository
        .set_notification_email(email.as_deref())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(ApiJson(ApiResponse::success(email)))
}

/// `POST /api/admin/settings/test-notification-email`
///
/// Send a test email to the configured notification address.
/// Returns an error if SMTP is not configured or sending fails.
pub async fn test_notification_email(
    AdminUser(_admin): AdminUser,
    State(state): State<AppState>,
) -> Result<ApiJson<ApiResponse<String>>, ApiError> {
    let svc = state
        .email_service
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("SMTP is not configured on the server.".to_string()))?;

    let to = state
        .setup_repository
        .get_notification_email()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::BadRequest("No notification email address configured.".to_string()))?;

    svc.send(
        &to,
        "Test notification — HungryAyam",
        "<p>This is a test email from HungryAyam. SMTP is working correctly.</p>".to_string(),
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(ApiJson(ApiResponse::success(format!("Test email sent to {to}"))))
}

/// `PUT /api/admin/settings/editor-domain`
///
/// Set or clear the editor email domain.
pub async fn set_editor_domain(
    AdminUser(_admin): AdminUser,
    State(state): State<AppState>,
    ApiJson(request): ApiJson<SetEditorDomainRequest>,
) -> Result<ApiJson<ApiResponse<Option<String>>>, ApiError> {
    let domain = request.domain.map(|d| d.trim().to_lowercase());

    state
        .setup_repository
        .set_editor_email_domain(domain.as_deref())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(ApiJson(ApiResponse::success(domain)))
}
