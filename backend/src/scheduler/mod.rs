//! Background scheduler for periodic tasks.
//!
//! Uses a **computed wake-up** approach instead of polling:
//!
//! 1. On startup (and after each task execution), query the DB for the next
//!    event time — the earliest of:
//!    - Next `menu_reset_time` that hasn't run today (converted to UTC)
//!    - Earliest `end_date` of any `Open` session where `allow_late = false`
//!      and the restaurant has `auto_close_session = true`
//! 2. `tokio::time::sleep_until(next_event)` — sleep precisely until then
//!    (capped at 5 minutes to catch drift or newly inserted events)
//! 3. `tokio::sync::Notify` wakes the scheduler early when relevant data
//!    changes (session created/updated/closed, order settings changed)
//! 4. The loop uses `tokio::select!` to race sleep vs notify
//!
//! Tasks:
//! 1. **Menu auto-reset** — At a configured time each day, set all items in
//!    non-permanent menus to `is_available = false`. Uses `scheduled_task_log`
//!    to prevent double-execution on the same calendar day.
//! 2. **Session auto-close** — When an order session's `end_date` passes and
//!    the restaurant has `auto_close_session = true`, transition the session
//!    from `Open` to `Closed`.
//! 3. **SMS fallback** — When the iPhone Shortcut did not confirm the SMS in
//!    time, send it through the OVH SMS API instead.

use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{sleep_until, Instant};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::features::{
    email::EmailService,
    order::domain::order_session::OrderSessionStatus,
    sms::OvhSmsClient,
};

/// Maximum time the scheduler will sleep before waking up to re-check.
/// This caps drift from events inserted without a notify (e.g. direct DB edits).
const MAX_SLEEP_SECS: u64 = 5 * 60; // 5 minutes

/// Fallback when the Shortcut received the email but never confirmed the SMS.
const FALLBACK_AFTER_RECEIVED_SECS: i64 = 5 * 60;
/// Fallback when the email was never received by the Shortcut. Must be longer
/// than the 5-minute `verify-notification` window so that the Shortcut can no
/// longer start sending once the fallback fires.
const FALLBACK_AFTER_UNRECEIVED_SECS: i64 = 6 * 60;
/// Delay between two OVH attempts for the same event.
const FALLBACK_RETRY_SECS: i64 = 60;
const FALLBACK_MAX_ATTEMPTS: i32 = 3;
/// Never send a fallback SMS for a notification older than this (e.g. after
/// downtime) — the order would be stale.
const FALLBACK_MAX_AGE_SECS: i64 = 60 * 60;

/// Resolve a system user for `updated_by` fields on scheduler-initiated writes.
/// Picks the first user in the database (typically the initial admin).
async fn resolve_system_user(pool: &PgPool) -> Option<Uuid> {
    sqlx::query_scalar!(r#"SELECT id FROM users ORDER BY created_at ASC LIMIT 1"#)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// Spawn the background scheduler as a Tokio task.
///
/// The scheduler runs indefinitely until the process exits. It uses the
/// provided `Notify` handle to wake early when relevant data changes.
pub fn spawn_scheduler(
    pool: PgPool,
    notify: Arc<Notify>,
    email_service: Option<EmailService>,
    notification_secret: Option<String>,
    sms_client: Option<OvhSmsClient>,
) {
    tokio::spawn(async move {
        info!("Scheduler started (sleep_until + Notify pattern)");

        // Wait for at least one user to exist (needed for updated_by)
        let system_user = loop {
            match resolve_system_user(&pool).await {
                Some(uid) => break uid,
                None => {
                    warn!("Scheduler: no users in database yet, retrying in 30s…");
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
            }
        };
        info!("Scheduler using system user: {}", system_user);

        loop {
            // 1. Run any tasks that are currently due
            let (reset_result, close_result, finish_result, fallback_result) = tokio::join!(
                run_menu_auto_reset(&pool, system_user),
                run_session_auto_close(&pool, system_user, email_service.as_ref(), notification_secret.as_deref()),
                run_session_auto_finish(&pool, system_user),
                run_sms_fallback(&pool, system_user, sms_client.as_ref()),
            );

            if let Err(e) = reset_result {
                error!("Scheduler: menu auto-reset failed: {e:#}");
            }
            if let Err(e) = close_result {
                error!("Scheduler: session auto-close failed: {e:#}");
            }
            if let Err(e) = finish_result {
                error!("Scheduler: session auto-finish failed: {e:#}");
            }
            if let Err(e) = fallback_result {
                error!("Scheduler: SMS fallback check failed: {e:#}");
            }

            // 2. Compute how long to sleep until the next event
            let sleep_duration = match compute_next_wake(&pool).await {
                Ok(d) => d,
                Err(e) => {
                    error!("Scheduler: failed to compute next wake time: {e:#}");
                    // Fallback: retry after the max sleep interval
                    std::time::Duration::from_secs(MAX_SLEEP_SECS)
                }
            };

            info!(
                "Scheduler: sleeping for {:.1}s until next event",
                sleep_duration.as_secs_f64()
            );

            let deadline = Instant::now() + sleep_duration;

            // 3. Race: sleep_until vs notify (early wake)
            tokio::select! {
                _ = sleep_until(deadline) => {
                    // Timer expired — time to check for due tasks
                }
                _ = notify.notified() => {
                    info!("Scheduler: woken early by notify");
                    // Data changed — re-evaluate what's due
                }
            }
        }
    });
}

// ─── Next-Wake Computation ────────────────────────────────────────────────

/// Compute the duration until the next scheduled event.
///
/// Queries both menu-reset candidates and session-close candidates, picks the
/// earliest, and returns the duration from now. Capped at `MAX_SLEEP_SECS`.
async fn compute_next_wake(pool: &PgPool) -> anyhow::Result<std::time::Duration> {
    let now_utc = Utc::now();
    let max_wake = now_utc + chrono::Duration::seconds(MAX_SLEEP_SECS as i64);
    let mut earliest = max_wake;

    // ── Menu reset candidates ─────────────────────────────────────────
    let reset_candidates = sqlx::query!(
        r#"
        SELECT restaurant_id,
               menu_reset_time as "menu_reset_time!: chrono::NaiveTime",
               timezone
        FROM restaurant_order_settings
        WHERE menu_reset_time IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await?;

    for row in &reset_candidates {
        let tz: chrono_tz::Tz = match row.timezone.parse() {
            Ok(tz) => tz,
            Err(_) => {
                warn!(
                    "Scheduler: invalid timezone '{}' for restaurant {}",
                    row.timezone, row.restaurant_id
                );
                continue;
            }
        };

        let local_now = now_utc.with_timezone(&tz);
        let local_date: NaiveDate = local_now.date_naive();
        let local_time = local_now.time();

        // Check if today's reset has already been executed
        let already_done = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM scheduled_task_log
                WHERE restaurant_id = $1
                  AND task_kind = 'menu_reset'
                  AND last_executed_date = $2
            ) as "exists!: bool"
            "#,
            row.restaurant_id,
            local_date,
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        // Determine the next reset instant in local time
        let next_reset_local = if !already_done && local_time < row.menu_reset_time {
            // Still upcoming today
            local_date.and_time(row.menu_reset_time)
        } else {
            // Already done today or time has passed — schedule for tomorrow
            let tomorrow = local_date.succ_opt().unwrap_or(local_date);
            tomorrow.and_time(row.menu_reset_time)
        };

        // Convert local NaiveDateTime → UTC via the timezone
        let maybe_utc = match next_reset_local.and_local_timezone(tz) {
            chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
            chrono::LocalResult::Ambiguous(dt, _) => Some(dt.with_timezone(&Utc)),
            chrono::LocalResult::None => None,
        };
        if let Some(utc_dt) = maybe_utc {
            if utc_dt < earliest {
                earliest = utc_dt;
            }
        }
    }

    // ── Session auto-close candidates ─────────────────────────────────
    // Find the earliest end_date among open sessions that qualify for auto-close.
    let earliest_session_end = sqlx::query_scalar!(
        r#"
        SELECT MIN(os.end_date) as "min_end: chrono::DateTime<Utc>"
        FROM order_sessions os
        JOIN restaurant_order_settings ros ON ros.restaurant_id = os.restaurant_id
        WHERE os.status = $1
          AND os.allow_late = false
          AND ros.auto_close_session = true
        "#,
        OrderSessionStatus::Open.as_i16(),
    )
    .fetch_one(pool)
    .await?;

    if let Some(end_dt) = earliest_session_end {
        if end_dt < earliest {
            earliest = end_dt;
        }
    }

    // ── Session auto-finish candidates ────────────────────────────────
    // Earliest pickup_time of any Confirmed session (auto-finish trigger).
    let earliest_finish = sqlx::query_scalar!(
        r#"
        SELECT MIN(pickup_time) as "min_pickup: chrono::DateTime<Utc>"
        FROM order_sessions
        WHERE status = $1
          AND pickup_time IS NOT NULL
        "#,
        OrderSessionStatus::Confirmed.as_i16(),
    )
    .fetch_one(pool)
    .await?;

    if let Some(pickup_dt) = earliest_finish {
        if pickup_dt < earliest {
            earliest = pickup_dt;
        }
    }

    // ── SMS fallback candidates ───────────────────────────────────────
    if let Some(due) = next_fallback_due(pool).await? {
        if due < earliest {
            earliest = due;
        }
    }

    // ── Compute the duration ──────────────────────────────────────────
    let diff = earliest - now_utc;
    let duration = if diff <= chrono::Duration::zero() {
        // Event is already due (or in the past) — wake immediately
        // but add a tiny delay to avoid a busy-spin if tasks keep failing
        std::time::Duration::from_millis(100)
    } else {
        let millis = diff.num_milliseconds().max(0) as u64;
        std::time::Duration::from_millis(millis)
    };

    // Cap at MAX_SLEEP_SECS
    let max_dur = std::time::Duration::from_secs(MAX_SLEEP_SECS);
    Ok(duration.min(max_dur))
}

// ─── Menu Auto-Reset ──────────────────────────────────────────────────────

/// Row returned by the menu-reset candidate query.
struct ResetCandidate {
    restaurant_id: Uuid,
    /// The restaurant's configured reset time (local), e.g. "06:00".
    menu_reset_time: chrono::NaiveTime,
    /// IANA timezone string.
    timezone: String,
}

async fn run_menu_auto_reset(pool: &PgPool, system_user: Uuid) -> anyhow::Result<()> {
    // Find restaurants that have a menu_reset_time configured
    let candidates = sqlx::query_as!(
        ResetCandidate,
        r#"
        SELECT restaurant_id,
               menu_reset_time as "menu_reset_time!: chrono::NaiveTime",
               timezone
        FROM restaurant_order_settings
        WHERE menu_reset_time IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await?;

    if candidates.is_empty() {
        return Ok(());
    }

    let now_utc = Utc::now();

    for candidate in candidates {
        if let Err(e) =
            process_menu_reset(pool, &candidate, now_utc, system_user).await
        {
            error!(
                "Scheduler: menu reset failed for restaurant {}: {e:#}",
                candidate.restaurant_id
            );
        }
    }

    Ok(())
}

async fn process_menu_reset(
    pool: &PgPool,
    candidate: &ResetCandidate,
    now_utc: chrono::DateTime<Utc>,
    system_user: Uuid,
) -> anyhow::Result<()> {
    // Parse the timezone
    let tz: chrono_tz::Tz = candidate
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid timezone: {}", candidate.timezone))?;

    // Convert current UTC time to the restaurant's local time
    let local_now = now_utc.with_timezone(&tz);
    let local_date: NaiveDate = local_now.date_naive();
    let local_time = local_now.time();

    // Has the reset time passed today?
    if local_time < candidate.menu_reset_time {
        return Ok(()); // Not yet time to reset today
    }

    // Check the task log to see if we already reset today
    let already_done = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM scheduled_task_log
            WHERE restaurant_id = $1
              AND task_kind = 'menu_reset'
              AND last_executed_date = $2
        ) as "exists!: bool"
        "#,
        candidate.restaurant_id,
        local_date,
    )
    .fetch_one(pool)
    .await?;

    if already_done {
        return Ok(()); // Already reset today
    }

    // Perform the reset: set all items in non-permanent menus to unavailable
    let result = sqlx::query!(
        r#"
        UPDATE menu_section_items
        SET is_available = false,
            updated_at = NOW(),
            updated_by = $2
        WHERE section_id IN (
            SELECT ms.id FROM menu_sections ms
            JOIN menus m ON ms.menu_id = m.id
            WHERE m.restaurant_id = $1
              AND m.permanent = false
        )
        AND is_available = true
        "#,
        candidate.restaurant_id,
        system_user,
    )
    .execute(pool)
    .await?;

    let rows_affected = result.rows_affected();

    // Record in the task log (upsert to handle race conditions)
    sqlx::query!(
        r#"
        INSERT INTO scheduled_task_log (restaurant_id, task_kind, last_executed_date, last_executed_at)
        VALUES ($1, 'menu_reset', $2, NOW())
        ON CONFLICT (restaurant_id, task_kind, last_executed_date)
        DO UPDATE SET last_executed_at = NOW(), updated_at = NOW()
        "#,
        candidate.restaurant_id,
        local_date,
    )
    .execute(pool)
    .await?;

    if rows_affected > 0 {
        info!(
            "Scheduler: auto-reset {} item(s) for restaurant {}",
            rows_affected, candidate.restaurant_id
        );
    }

    Ok(())
}

// ─── Session Auto-Close ───────────────────────────────────────────────────

/// Row returned by the auto-close candidate query.
struct CloseCandidate {
    session_id: Uuid,
    restaurant_id: Uuid,
    pickup_time: Option<chrono::DateTime<Utc>>,
    notify_on_session_close: bool,
    restaurant_name: String,
    restaurant_phone: Option<String>,
    sms_template: Option<String>,
}

async fn run_session_auto_close(
    pool: &PgPool,
    system_user: Uuid,
    email_service: Option<&EmailService>,
    notification_secret: Option<&str>,
) -> anyhow::Result<()> {
    let candidates = sqlx::query_as!(
        CloseCandidate,
        r#"
        SELECT os.id            as session_id,
               os.restaurant_id as restaurant_id,
               os.pickup_time   as "pickup_time?: chrono::DateTime<Utc>",
               ros.notify_on_session_close,
               r.name           as restaurant_name,
               r.sms_phone_number as "restaurant_phone?: String",
               r.sms_message_template as "sms_template?: String"
        FROM order_sessions os
        JOIN restaurant_order_settings ros ON ros.restaurant_id = os.restaurant_id
        JOIN restaurants r ON r.id = os.restaurant_id
        WHERE os.status = $1
          AND os.end_date <= NOW()
          AND os.allow_late = false
          AND ros.auto_close_session = true
        "#,
        OrderSessionStatus::Open.as_i16(),
    )
    .fetch_all(pool)
    .await?;

    if candidates.is_empty() {
        return Ok(());
    }

    // Fetch the notification address once (global setting)
    let notification_email: Option<String> = if email_service.is_some() {
        sqlx::query_scalar!(r#"SELECT notification_email FROM app_settings WHERE id = 1"#)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten()
    } else {
        None
    };

    for candidate in candidates {
        if let Err(e) = close_session(pool, &candidate, system_user, email_service, notification_email.as_deref(), notification_secret).await {
            error!(
                "Scheduler: auto-close failed for session {}: {e:#}",
                candidate.session_id
            );
        }
    }

    Ok(())
}

async fn close_session(
    pool: &PgPool,
    candidate: &CloseCandidate,
    system_user: Uuid,
    email_service: Option<&EmailService>,
    notification_email: Option<&str>,
    notification_secret: Option<&str>,
) -> anyhow::Result<()> {
    let result = sqlx::query!(
        r#"
        UPDATE order_sessions
        SET status = $1,
            updated_at = NOW(),
            updated_by = $3
        WHERE id = $2
          AND status = $4
        "#,
        OrderSessionStatus::Closed.as_i16(),
        candidate.session_id,
        system_user,
        OrderSessionStatus::Open.as_i16(),
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(());
    }

    info!(
        "Scheduler: auto-closed session {} for restaurant {}",
        candidate.session_id, candidate.restaurant_id
    );

    // Send notification email if configured
    if candidate.notify_on_session_close {
        if let (Some(svc), Some(to)) = (email_service, notification_email) {
            if let Err(e) = crate::features::notification::send_order_notification(
                pool,
                candidate.session_id,
                candidate.pickup_time,
                &candidate.restaurant_name,
                candidate.restaurant_phone.as_deref(),
                svc,
                to,
                notification_secret,
                candidate.sms_template.as_deref(),
            ).await {
                warn!("Scheduler: failed to send session-close notification: {e}");
            }
        }
    }

    Ok(())
}

// ─── SMS Fallback ─────────────────────────────────────────────────────────

/// Earliest instant at which a pending notification event becomes due for
/// the OVH fallback (`None` if nothing is pending).
async fn next_fallback_due(pool: &PgPool) -> anyhow::Result<Option<chrono::DateTime<Utc>>> {
    let due = sqlx::query_scalar!(
        r#"
        SELECT MIN(GREATEST(
                   CASE WHEN ne.email_received_at IS NOT NULL
                        THEN ne.email_received_at + make_interval(secs => $1)
                        ELSE ne.email_sent_at + make_interval(secs => $2)
                   END,
                   COALESCE(ne.fallback_attempted_at + make_interval(secs => $3), '-infinity')
               )) as "due: chrono::DateTime<Utc>"
        FROM notification_events ne
        JOIN order_sessions os ON os.id = ne.session_id
        WHERE ne.sms_sent_at IS NULL
          AND ne.fallback_sent_at IS NULL
          AND ne.fallback_attempts < $4
          AND ne.email_sent_at > NOW() - make_interval(secs => $5)
          AND os.status = ANY($6)
        "#,
        FALLBACK_AFTER_RECEIVED_SECS as f64,
        FALLBACK_AFTER_UNRECEIVED_SECS as f64,
        FALLBACK_RETRY_SECS as f64,
        FALLBACK_MAX_ATTEMPTS,
        FALLBACK_MAX_AGE_SECS as f64,
        &[OrderSessionStatus::Closed.as_i16(), OrderSessionStatus::Requested.as_i16()],
    )
    .fetch_one(pool)
    .await?;
    Ok(due)
}

/// For notification events whose SMS was not confirmed by the iPhone Shortcut
/// in time — either the email was received but no confirmation came within
/// 5 minutes, or the email never reached the Shortcut — send the SMS through
/// OVH and transition the session to SmsSent.
///
/// Sessions that were already moved on manually (SmsSent, Confirmed,
/// Cancelled…) are skipped. Failed attempts are retried up to
/// `FALLBACK_MAX_ATTEMPTS` times.
pub(crate) async fn run_sms_fallback(
    pool: &PgPool,
    system_user: Uuid,
    sms_client: Option<&OvhSmsClient>,
) -> anyhow::Result<()> {
    let overdue = sqlx::query!(
        r#"
        SELECT ne.sig, ne.session_id, ne.phone, ne.time_str, ne.orders, ne.message
        FROM notification_events ne
        JOIN order_sessions os ON os.id = ne.session_id
        WHERE ne.sms_sent_at IS NULL
          AND ne.fallback_sent_at IS NULL
          AND ne.fallback_attempts < $4
          AND ne.email_sent_at > NOW() - make_interval(secs => $5)
          AND os.status = ANY($6)
          AND (
                (ne.email_received_at IS NOT NULL
                 AND ne.email_received_at <= NOW() - make_interval(secs => $1))
             OR (ne.email_received_at IS NULL
                 AND ne.email_sent_at <= NOW() - make_interval(secs => $2))
          )
          AND (ne.fallback_attempted_at IS NULL
               OR ne.fallback_attempted_at <= NOW() - make_interval(secs => $3))
        "#,
        FALLBACK_AFTER_RECEIVED_SECS as f64,
        FALLBACK_AFTER_UNRECEIVED_SECS as f64,
        FALLBACK_RETRY_SECS as f64,
        FALLBACK_MAX_ATTEMPTS,
        FALLBACK_MAX_AGE_SECS as f64,
        &[OrderSessionStatus::Closed.as_i16(), OrderSessionStatus::Requested.as_i16()],
    )
    .fetch_all(pool)
    .await?;

    if overdue.is_empty() {
        return Ok(());
    }

    let Some(client) = sms_client else {
        warn!(
            "Scheduler: {} SMS notification(s) not confirmed by the Shortcut, but OVH fallback is not configured",
            overdue.len()
        );
        return Ok(());
    };

    for row in overdue {
        // Claim the event first: once fallback_attempts > 0, verify-notification
        // tells the Shortcut not to send, so the SMS can't go out twice.
        let claimed = sqlx::query_scalar!(
            r#"
            UPDATE notification_events
            SET fallback_attempts = fallback_attempts + 1,
                fallback_attempted_at = NOW()
            WHERE sig = $1
              AND sms_sent_at IS NULL
              AND fallback_sent_at IS NULL
            RETURNING fallback_attempts
            "#,
            row.sig,
        )
        .fetch_optional(pool)
        .await?;
        let Some(attempt) = claimed else { continue };

        warn!(
            "Scheduler: SMS not confirmed by Shortcut for session {} — OVH fallback (attempt {attempt}/{FALLBACK_MAX_ATTEMPTS})",
            row.session_id
        );

        // Old rows (before `message` existed) fall back to the default format
        let msg = row.message.unwrap_or_else(|| {
            crate::features::notification::render_sms_message(None, &row.time_str, &row.orders)
        });

        let result = if crate::features::sms::normalize_phone(&row.phone).is_none() {
            // Not retryable — give up straight away
            sqlx::query!(
                "UPDATE notification_events SET fallback_attempts = $2 WHERE sig = $1",
                row.sig,
                FALLBACK_MAX_ATTEMPTS,
            )
            .execute(pool)
            .await?;
            Err(anyhow::anyhow!("restaurant has no valid SMS phone number ('{}')", row.phone))
        } else {
            client.send(&row.phone, &msg).await
        };

        match result {
            Ok(_) => {
                sqlx::query!(
                    "UPDATE notification_events SET fallback_sent_at = NOW(), fallback_error = NULL WHERE sig = $1",
                    row.sig,
                )
                .execute(pool)
                .await?;

                // Transition Closed or Requested → SmsSent
                sqlx::query!(
                    r#"
                    UPDATE order_sessions
                    SET status     = $1,
                        updated_at = NOW(),
                        updated_by = $2
                    WHERE id = $3
                      AND status = ANY($4)
                    "#,
                    OrderSessionStatus::SmsSent.as_i16(),
                    system_user,
                    row.session_id,
                    &[OrderSessionStatus::Closed.as_i16(), OrderSessionStatus::Requested.as_i16()],
                )
                .execute(pool)
                .await?;
                info!("Scheduler: OVH fallback SMS sent for session {}", row.session_id);
            }
            Err(e) => {
                error!("Scheduler: OVH fallback failed for session {}: {e:#}", row.session_id);
                sqlx::query!(
                    "UPDATE notification_events SET fallback_error = $2 WHERE sig = $1",
                    row.sig,
                    format!("{e:#}"),
                )
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(())
}

// ─── Session Auto-Finish ──────────────────────────────────────────────────

/// Auto-finish Confirmed sessions whose pickup_time has passed.
async fn run_session_auto_finish(pool: &PgPool, system_user: Uuid) -> anyhow::Result<()> {
    let result = sqlx::query!(
        r#"
        UPDATE order_sessions
        SET status     = $1,
            updated_at = NOW(),
            updated_by = $2
        WHERE status = $3
          AND pickup_time IS NOT NULL
          AND pickup_time <= NOW()
        "#,
        OrderSessionStatus::Finished.as_i16(),
        system_user,
        OrderSessionStatus::Confirmed.as_i16(),
    )
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 {
        info!(
            "Scheduler: auto-finished {} session(s) after pickup_time",
            result.rows_affected()
        );
    }

    Ok(())
}
#[cfg(test)]
mod tests;
