use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Default session duration for guest (NameWithCookie) users: 30 days.
const GUEST_SESSION_DAYS: i64 = 30;

/// Default session duration for password-authenticated users: 7 days.
const PASSWORD_SESSION_DAYS: i64 = 7;

// ═══════════════════════════════════════════════════════════════════
// Domain
// ═══════════════════════════════════════════════════════════════════

/// A user session, stored in the `user_sessions` table.
///
/// Both `NameWithCookie` and `Password` users get a session after
/// authenticating. The session token is sent to the client as a cookie
/// (or via `Authorization: Bearer <token>` header) and is used to
/// identify the user on subsequent requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

// ═══════════════════════════════════════════════════════════════════
// Repository
// ═══════════════════════════════════════════════════════════════════

#[derive(Clone)]
pub struct SessionRepository {
    pool: PgPool,
}

impl SessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new session for a guest (NameWithCookie) user.
    /// Uses a long-lived expiration (30 days).
    pub async fn create_guest_session(&self, user_id: Uuid) -> Result<UserSession> {
        self.create(user_id, GUEST_SESSION_DAYS).await
    }

    /// Create a new session for a password-authenticated user.
    /// Uses a shorter expiration (7 days).
    pub async fn create_password_session(&self, user_id: Uuid) -> Result<UserSession> {
        self.create(user_id, PASSWORD_SESSION_DAYS).await
    }

    /// Create a session with a custom duration (in days).
    async fn create(&self, user_id: Uuid, duration_days: i64) -> Result<UserSession> {
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::days(duration_days);

        let session = sqlx::query_as!(
            UserSession,
            r#"
            INSERT INTO user_sessions (user_id, token, expires_at)
            VALUES ($1, $2, $3)
            RETURNING id, user_id, token, expires_at, created_at
            "#,
            user_id,
            token,
            expires_at
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(session)
    }

    /// Look up a session by its token.
    /// Only returns the session if it has not expired.
    pub async fn get_by_token(&self, token: &str) -> Result<Option<UserSession>> {
        let session = sqlx::query_as!(
            UserSession,
            r#"
            SELECT id, user_id, token, expires_at, created_at
            FROM user_sessions
            WHERE token = $1
              AND expires_at > NOW()
            "#,
            token
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(session)
    }

    /// Delete a single session by its token (logout).
    pub async fn delete_by_token(&self, token: &str) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM user_sessions WHERE token = $1",
            token
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete all sessions for a given user (logout everywhere / forced re-login).
    pub async fn delete_by_user_id(&self, user_id: Uuid) -> Result<u64> {
        let result = sqlx::query!(
            "DELETE FROM user_sessions WHERE user_id = $1",
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Remove all expired sessions from the table.
    /// Call this periodically (e.g., from a background task) to keep the table tidy.
    pub async fn cleanup_expired(&self) -> Result<u64> {
        let result = sqlx::query!(
            "DELETE FROM user_sessions WHERE expires_at <= NOW()"
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Extend (renew) an existing session's expiration.
    /// Useful for "sliding window" session expiry on user activity.
    pub async fn renew(&self, token: &str, duration_days: i64) -> Result<Option<UserSession>> {
        let new_expires_at = Utc::now() + Duration::days(duration_days);

        let session = sqlx::query_as!(
            UserSession,
            r#"
            UPDATE user_sessions
            SET expires_at = $1
            WHERE token = $2
              AND expires_at > NOW()
            RETURNING id, user_id, token, expires_at, created_at
            "#,
            new_expires_at,
            token
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(session)
    }
}