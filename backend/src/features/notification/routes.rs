use axum::{Router, routing::get, extract::{Query, State}, Json};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use ts_rs::TS;

use crate::{
    state::AppState,
    features::order::domain::order_session::OrderSessionStatus,
    types::response::ApiResponse,
};

use super::verify;

pub fn notification_routes() -> Router<AppState> {
    Router::new()
        .route("/api/verify-notification", get(verify_notification))
        .route("/api/confirm-sms", get(confirm_sms))
}

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct VerifyParams {
    pub sig: String,
    pub ts: i64,
    pub phone: String,
    pub time: String,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmSmsParams {
    pub sig: String,
}

// ── Response ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct NotificationVerifyResponse {
    pub valid: bool,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// Called by iPhone Shortcuts when the notification email is received.
/// Verifies the HMAC signature and marks the event as received.
async fn verify_notification(
    State(state): State<AppState>,
    Query(params): Query<VerifyParams>,
) -> Json<ApiResponse<NotificationVerifyResponse>> {
    let Some(secret) = &state.notification_secret else {
        warn!("verify-notification called but NOTIFICATION_SECRET is not set");
        return Json(ApiResponse::error("Notification verification not configured".to_string()));
    };

    // 1. Verify HMAC + timestamp (5 min window)
    if let Err(e) = verify(secret, &params.sig, params.ts, &params.phone, &params.time, 300) {
        warn!("verify-notification rejected: {e}");
        return Json(ApiResponse::success(NotificationVerifyResponse { valid: false }));
    }

    // 2. Mark email_received_at — only if not already set (prevent double-trigger)
    let result = sqlx::query!(
        r#"
        UPDATE notification_events
        SET email_received_at = NOW()
        WHERE sig = $1
          AND email_received_at IS NULL
        "#,
        params.sig,
    )
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            info!("verify-notification: email received for sig={}", &params.sig[..8]);
        }
        Ok(_) => {
            // Already received or sig not found — still return valid=true so Shortcut proceeds
            info!("verify-notification: sig={} already received or not found", &params.sig[..8]);
        }
        Err(e) => {
            warn!("verify-notification: DB error: {e}");
            return Json(ApiResponse::error("Database error".to_string()));
        }
    }

    Json(ApiResponse::success(NotificationVerifyResponse { valid: true }))
}

/// Called by iPhone Shortcuts after the SMS has been sent successfully.
/// Marks the event as SMS-sent and transitions the session to Requested.
async fn confirm_sms(
    State(state): State<AppState>,
    Query(params): Query<ConfirmSmsParams>,
) -> Json<ApiResponse<NotificationVerifyResponse>> {
    // Look up the event
    let event = sqlx::query!(
        r#"
        SELECT session_id, email_received_at, sms_sent_at
        FROM notification_events
        WHERE sig = $1
        "#,
        params.sig,
    )
    .fetch_optional(&state.db)
    .await;

    let event = match event {
        Ok(Some(e)) => e,
        Ok(None) => {
            warn!("confirm-sms: sig not found: {}", &params.sig[..8.min(params.sig.len())]);
            return Json(ApiResponse::success(NotificationVerifyResponse { valid: false }));
        }
        Err(e) => {
            warn!("confirm-sms: DB error: {e}");
            return Json(ApiResponse::error("Database error".to_string()));
        }
    };

    if event.sms_sent_at.is_some() {
        info!("confirm-sms: already confirmed for sig={}", &params.sig[..8]);
        return Json(ApiResponse::success(NotificationVerifyResponse { valid: true }));
    }

    // Mark sms_sent_at
    if let Err(e) = sqlx::query!(
        "UPDATE notification_events SET sms_sent_at = NOW() WHERE sig = $1",
        params.sig,
    )
    .execute(&state.db)
    .await
    {
        warn!("confirm-sms: failed to set sms_sent_at: {e}");
        return Json(ApiResponse::error("Database error".to_string()));
    }

    // Transition Closed or Requested → SmsSent
    let system_user = sqlx::query_scalar!(
        "SELECT id FROM users ORDER BY created_at ASC LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some(uid) = system_user {
        // Accept both Closed (auto-close path) and Requested (manual Send Request path)
        let _ = sqlx::query!(
            r#"
            UPDATE order_sessions
            SET status     = $1,
                updated_at = NOW(),
                updated_by = $2
            WHERE id = $3
              AND status = ANY($4)
            "#,
            OrderSessionStatus::SmsSent.as_i16(),
            uid,
            event.session_id,
            &[OrderSessionStatus::Closed.as_i16(), OrderSessionStatus::Requested.as_i16()],
        )
        .execute(&state.db)
        .await;
        state.scheduler_notify.notify_one();
    }

    info!("confirm-sms: SMS confirmed, session {} → Requested", event.session_id);
    Json(ApiResponse::success(NotificationVerifyResponse { valid: true }))
}
