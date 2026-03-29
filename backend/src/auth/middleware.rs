use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts},
};

use crate::{
    errors::api_errors::ApiError,
    features::user::domain::User,
    state::AppState,
    types::role::UserRole,
};

/// Name of the cookie that carries the session token.
pub const SESSION_COOKIE_NAME: &str = "session_token";

/// Name of the cookie that tracks site-level access (shared password gate).
pub const SITE_ACCESS_COOKIE_NAME: &str = "site_access";

// ═══════════════════════════════════════════════════════════════════
// AuthUser — any authenticated user (guest or password)
// ═══════════════════════════════════════════════════════════════════

/// Extractor that resolves the current user from a session token.
///
/// The token is read from (in order of precedence):
/// 1. The `Authorization: Bearer <token>` header
/// 2. The `session_token` cookie
///
/// If no valid session is found, the request is rejected with `401 Unauthorized`.
///
/// # Example
///
/// ```rust,ignore
/// async fn my_handler(AuthUser(user): AuthUser) -> impl IntoResponse {
///     format!("Hello, {}!", user.name)
/// }
/// ```
pub struct AuthUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_token(parts).ok_or(ApiError::Unauthorized)?;

        let session = state
            .session_repository
            .get_by_token(&token)
            .await
            .map_err(|_| ApiError::Unauthorized)?
            .ok_or(ApiError::Unauthorized)?;

        let user = state
            .user_service
            .get_user(session.user_id)
            .await
            .map_err(|_| ApiError::Unauthorized)?
            .ok_or(ApiError::Unauthorized)?;

        Ok(AuthUser(user))
    }
}

// ═══════════════════════════════════════════════════════════════════
// EditorUser — users with role = Editor or Admin
// ═══════════════════════════════════════════════════════════════════

/// Extractor that resolves the current user and verifies they have at least
/// `Editor` role (i.e. `Editor` or `Admin`).
///
/// Rejects with:
/// - `401 Unauthorized` if no valid session
/// - `403 Forbidden` if the user's role is below Editor
///
/// # Example
///
/// ```rust,ignore
/// async fn editor_handler(EditorUser(user): EditorUser) -> impl IntoResponse {
///     format!("Hello editor {}!", user.name)
/// }
/// ```
pub struct EditorUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for EditorUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;

        match &user.role {
            Some(role) if role.is_editor_or_above() => Ok(EditorUser(user)),
            _ => Err(ApiError::Forbidden),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// AdminUser — only users with role = Admin
// ═══════════════════════════════════════════════════════════════════

/// Extractor that resolves the current user and verifies they have `Admin` role.
///
/// Rejects with:
/// - `401 Unauthorized` if no valid session
/// - `403 Forbidden` if the user is not an Admin
///
/// # Example
///
/// ```rust,ignore
/// async fn admin_only_handler(AdminUser(admin): AdminUser) -> impl IntoResponse {
///     format!("Welcome admin {}!", admin.name)
/// }
/// ```
pub struct AdminUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;

        match &user.role {
            Some(role) if role.is_admin() => Ok(AdminUser(user)),
            _ => Err(ApiError::Forbidden),
        }
    }
}


// ═══════════════════════════════════════════════════════════════════
// SiteAccess — verifies the visitor passed the site-level password gate
// ═══════════════════════════════════════════════════════════════════

/// Extractor that verifies the caller has site-level access.
///
/// Access is granted if **either** condition is met:
/// 1. The caller has a valid user session (authenticated users have
///    implicit site access — they passed the gate at some point).
/// 2. The `site_access` cookie matches the `access_hash` stored in
///    `app_settings`.
///
/// Rejects with `403 Forbidden` if neither condition is satisfied.
///
/// Use this on routes that should only be available to visitors who
/// entered the shared site password (User-level access). Routes that
/// serve public statistics should NOT use this extractor.
///
/// # Example
///
/// ```rust,ignore
/// async fn protected_route(
///     _site: SiteAccess,
///     State(state): State<AppState>,
/// ) -> impl IntoResponse {
///     "You have site access!"
/// }
/// ```
pub struct SiteAccess;

#[async_trait]
impl FromRequestParts<AppState> for SiteAccess {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 1. Authenticated users have implicit site access
        if let Some(token) = extract_token(parts) {
            if state
                .session_repository
                .get_by_token(&token)
                .await
                .ok()
                .flatten()
                .is_some()
            {
                return Ok(SiteAccess);
            }
        }

        // 2. Otherwise, check the site_access cookie against stored hash
        let cookie_value =
            extract_cookie(parts, SITE_ACCESS_COOKIE_NAME).ok_or(ApiError::Forbidden)?;

        let stored_hash = state
            .setup_repository
            .get_access_hash()
            .await
            .map_err(|_| ApiError::Forbidden)?
            .ok_or(ApiError::Forbidden)?;

        if cookie_value == *stored_hash {
            Ok(SiteAccess)
        } else {
            Err(ApiError::Forbidden)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helper: extract session token from request
// ═══════════════════════════════════════════════════════════════════

/// Attempt to extract the session token from the request.
///
/// Checks in order:
/// 1. `Authorization: Bearer <token>` header
/// 2. `session_token=<token>` cookie
fn extract_token(parts: &Parts) -> Option<String> {
    // 1. Try Authorization header
    if let Some(auth_header) = parts.headers.get(header::AUTHORIZATION) {
        if let Ok(value) = auth_header.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                let token = token.trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }

    // 2. Try cookie
    extract_cookie(parts, SESSION_COOKIE_NAME)
}

/// Extract a named cookie value from the request headers.
fn extract_cookie(parts: &Parts, name: &str) -> Option<String> {
    if let Some(cookie_header) = parts.headers.get(header::COOKIE) {
        if let Ok(cookies) = cookie_header.to_str() {
            for cookie in cookies.split(';') {
                let cookie = cookie.trim();
                if let Some(rest) = cookie.strip_prefix(name) {
                    if let Some(value) = rest.strip_prefix('=') {
                        let value = value.trim();
                        if !value.is_empty() {
                            return Some(value.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Helper to build a `Set-Cookie` header value for the session token.
///
/// Sets `HttpOnly`, `SameSite=Lax`, and `Path=/`.
/// In production, you'd also add `Secure` and tune `Max-Age`.
pub fn build_session_cookie(token: &str, max_age_days: i64) -> String {
    let max_age_secs = max_age_days * 24 * 60 * 60;
    let secure = if std::env::var("SECURE_COOKIES").as_deref() == Ok("false") { "" } else { "; Secure" };
    format!(
        "{}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{}",
        SESSION_COOKIE_NAME, token, max_age_secs, secure
    )
}

/// Helper to build a `Set-Cookie` header value that clears the session cookie.
pub fn build_clear_session_cookie() -> String {
    let secure = if std::env::var("SECURE_COOKIES").as_deref() == Ok("false") { "" } else { "; Secure" };
    format!(
        "{}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0{}",
        SESSION_COOKIE_NAME, secure
    )
}

/// Helper to build a `Set-Cookie` header value for the site-access cookie.
///
/// The cookie value is the `access_token` from `app_settings`.
/// Sets `HttpOnly`, `SameSite=Lax`, and `Path=/`.
pub fn build_site_access_cookie(access_token: &str, max_age_days: i64) -> String {
    let max_age_secs = max_age_days * 24 * 60 * 60;
    let secure = if std::env::var("SECURE_COOKIES").as_deref() == Ok("false") { "" } else { "; Secure" };
    format!(
        "{}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{}",
        SITE_ACCESS_COOKIE_NAME, access_token, max_age_secs, secure
    )
}

/// Helper to build a `Set-Cookie` header value that clears the site-access cookie.
pub fn build_clear_site_access_cookie() -> String {
    let secure = if std::env::var("SECURE_COOKIES").as_deref() == Ok("false") { "" } else { "; Secure" };
    format!(
        "{}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0{}",
        SITE_ACCESS_COOKIE_NAME, secure
    )
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue, Request};

    fn parts_with_headers(headers: HeaderMap) -> Parts {
        let mut builder = Request::builder();
        for (key, value) in headers.iter() {
            builder = builder.header(key, value);
        }
        let (parts, _) = builder.body(()).unwrap().into_parts();
        parts
    }

    #[test]
    fn test_extract_token_from_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer abc-123-token"),
        );
        let parts = parts_with_headers(headers);
        assert_eq!(extract_token(&parts), Some("abc-123-token".to_string()));
    }

    #[test]
    fn test_extract_token_from_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("other=foo; session_token=my-session-tok; z=bar"),
        );
        let parts = parts_with_headers(headers);
        assert_eq!(
            extract_token(&parts),
            Some("my-session-tok".to_string())
        );
    }

    #[test]
    fn test_extract_site_access_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("site_access=my-access-token; other=x"),
        );
        let parts = parts_with_headers(headers);
        assert_eq!(
            extract_cookie(&parts, SITE_ACCESS_COOKIE_NAME),
            Some("my-access-token".to_string())
        );
    }

    #[test]
    fn test_bearer_takes_precedence_over_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer from-header"),
        );
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("session_token=from-cookie"),
        );
        let parts = parts_with_headers(headers);
        assert_eq!(extract_token(&parts), Some("from-header".to_string()));
    }

    #[test]
    fn test_no_token() {
        let parts = parts_with_headers(HeaderMap::new());
        assert_eq!(extract_token(&parts), None);
    }

    #[test]
    fn test_empty_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer "),
        );
        let parts = parts_with_headers(headers);
        assert_eq!(extract_token(&parts), None);
    }

    #[test]
    fn test_build_session_cookie() {
        let cookie = build_session_cookie("tok123", 7);
        assert!(cookie.contains("session_token=tok123"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Path=/"));
        assert!(cookie.contains("Max-Age=604800")); // 7 * 86400
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn test_build_clear_session_cookie() {
        let cookie = build_clear_session_cookie();
        assert!(cookie.contains("session_token=;"));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn test_build_site_access_cookie() {
        let cookie = build_site_access_cookie("tok-abc", 365);
        assert!(cookie.contains("site_access=tok-abc"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Path=/"));
        assert!(cookie.contains("Max-Age=31536000")); // 365 * 86400
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn test_build_clear_site_access_cookie() {
        let cookie = build_clear_site_access_cookie();
        assert!(cookie.contains("site_access=;"));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("Secure"));
    }
}
