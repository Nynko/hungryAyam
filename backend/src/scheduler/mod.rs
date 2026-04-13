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
};

/// Maximum time the scheduler will sleep before waking up to re-check.
/// This caps drift from events inserted without a notify (e.g. direct DB edits).
const MAX_SLEEP_SECS: u64 = 5 * 60; // 5 minutes

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
pub fn spawn_scheduler(pool: PgPool, notify: Arc<Notify>, email_service: Option<EmailService>, notification_secret: Option<String>) {
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
                run_sms_fallback(&pool, system_user),
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
               r.phone_number   as "restaurant_phone?: String"
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

    // Fetch notification settings once (global settings)
    let (notification_email, sms_template): (Option<String>, Option<String>) = if email_service.is_some() {
        let row = sqlx::query!(
            r#"SELECT notification_email, sms_message_template FROM app_settings WHERE id = 1"#
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        match row {
            Some(r) => (r.notification_email, r.sms_message_template),
            None => (None, None),
        }
    } else {
        (None, None)
    };

    for candidate in candidates {
        if let Err(e) = close_session(pool, &candidate, system_user, email_service, notification_email.as_deref(), notification_secret, sms_template.as_deref()).await {
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
    sms_template: Option<&str>,
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
            // Fetch order summary for this session
            // Fetch aggregated order: total quantity per item across all users
            let rows = sqlx::query!(
                r#"
                SELECT i.name as item_name,
                       COUNT(oi.id) as "qty!: i64"
                FROM orders o
                JOIN order_items oi ON oi.order_id = o.id
                JOIN items i ON i.id = oi.item_id
                WHERE o.session_id = $1
                GROUP BY i.name
                ORDER BY i.name
                "#,
                candidate.session_id,
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let pickup = candidate.pickup_time
                .map(|t| t.format("%H:%M").to_string())
                .unwrap_or_else(|| "not set".to_string());

            let phone = candidate.restaurant_phone.as_deref().unwrap_or("—");

            let mut order_lines = String::new();
            for row in &rows {
                order_lines.push_str(&format!("{} {}\n", row.qty, row.item_name));
            }

            let orders_text = order_lines.trim_end().to_string();

            // Build signed body if secret is configured
            let (ts, sig_line) = if let Some(secret) = notification_secret {
                let ts = chrono::Utc::now().timestamp();
                let sig = crate::features::notification::sign(secret, ts, phone, &pickup);
                (ts, format!("TS:{ts}\nSIG:{sig}\n"))
            } else {
                (0, String::new())
            };

            let msg = crate::features::notification::render_sms_message(sms_template, &pickup, &orders_text);
            let body = format!(
                "{sig_line}RESTAURANT:{restaurant}\nPHONE:{phone}\nTIME:{pickup}\nBODY:{msg}",
                sig_line = sig_line,
                restaurant = candidate.restaurant_name,
                phone = phone,
                pickup = pickup,
                msg = msg,
            );

            if let Err(e) = svc.send_plain(to, "HungryAyam Order", body).await {
                warn!("Scheduler: failed to send session-close notification: {e}");
            } else if notification_secret.is_some() {
                // Insert notification event for tracking
                let sig = crate::features::notification::sign(
                    notification_secret.unwrap(), ts, phone, &pickup
                );
                if let Err(e) = sqlx::query!(
                    r#"
                    INSERT INTO notification_events
                        (sig, session_id, ts, phone, time_str, orders, restaurant)
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    ON CONFLICT (sig) DO NOTHING
                    "#,
                    sig,
                    candidate.session_id,
                    ts,
                    phone,
                    pickup,
                    orders_text,
                    candidate.restaurant_name,
                )
                .execute(pool)
                .await
                {
                    warn!("Scheduler: failed to insert notification_event: {e}");
                }
            }
        }
    }

    Ok(())
}

// ─── SMS Fallback ─────────────────────────────────────────────────────────

/// For notification events where the email was received but the SMS was not
/// confirmed within 5 minutes, trigger the OVH SMS fallback and transition
/// the session to Requested.
async fn run_sms_fallback(pool: &PgPool, system_user: Uuid) -> anyhow::Result<()> {
    let overdue = sqlx::query!(
        r#"
        SELECT sig, session_id, phone, time_str, orders, restaurant
        FROM notification_events
        WHERE email_received_at IS NOT NULL
          AND sms_sent_at IS NULL
          AND fallback_sent_at IS NULL
          AND email_received_at < NOW() - INTERVAL '5 minutes'
        "#,
    )
    .fetch_all(pool)
    .await?;

    if overdue.is_empty() {
        return Ok(());
    }

    // Fetch SMS template once for all overdue events
    let sms_template: Option<String> = sqlx::query_scalar!(
        r#"SELECT sms_message_template FROM app_settings WHERE id = 1"#
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .flatten();

    for row in overdue {
        warn!(
            "Scheduler: SMS not confirmed after 5 min for session {} — triggering OVH fallback",
            row.session_id
        );

        let msg = crate::features::notification::render_sms_message(
            sms_template.as_deref(),
            &row.time_str,
            &row.orders,
        );

        // TODO: send OVH SMS here
        info!(
            "Scheduler: [OVH STUB] would send SMS to {} — MSG: {}",
            row.phone, msg
        );

        // Mark fallback as sent
        let _ = sqlx::query!(
            "UPDATE notification_events SET fallback_sent_at = NOW() WHERE sig = $1",
            row.sig,
        )
        .execute(pool)
        .await;

        // Transition session Closed → Requested
        let _ = sqlx::query!(
            r#"
            UPDATE order_sessions
            SET status     = $1,
                updated_at = NOW(),
                updated_by = $3
            WHERE id = $2
              AND status = $4
            "#,
            OrderSessionStatus::Requested.as_i16(),
            row.session_id,
            system_user,
            OrderSessionStatus::Closed.as_i16(),
        )
        .execute(pool)
        .await;
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