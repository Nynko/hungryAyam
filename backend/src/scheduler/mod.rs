//! Background scheduler for periodic tasks.
//!
//! Runs a loop every 60 seconds that checks for:
//! 1. **Menu auto-reset** — Non-permanent menus belonging to restaurants that
//!    have a `menu_reset_time` configured. When the local time passes the
//!    configured reset time, all `menu_section_items` are set to
//!    `is_available = false`. A `scheduled_task_log` entry prevents
//!    double-execution on the same calendar day.
//! 2. **Session auto-close** — Order sessions in `Open` status whose
//!    `end_date` is in the past. If the restaurant has `auto_close_session`
//!    enabled, the session is transitioned to `Closed`.

use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::features::order::domain::order_session::OrderSessionStatus;

/// Interval between scheduler ticks (in seconds).
const TICK_INTERVAL_SECS: u64 = 60;

/// System user UUID used as `updated_by` for scheduler-initiated changes.
/// This is a well-known sentinel — it doesn't need to exist in the `users`
/// table because the FK on `updated_by` points to `users(id)` which may be
/// relaxed for system operations. If your schema enforces it, seed this user
/// during app setup.
///
/// We use the first admin user instead — resolved at runtime.
async fn resolve_system_user(pool: &PgPool) -> Option<Uuid> {
    // Pick the first admin user (role = 0 is typically Admin in your schema)
    let row = sqlx::query_scalar!(
        r#"SELECT id FROM users ORDER BY created_at ASC LIMIT 1"#
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    row
}

/// Spawn the background scheduler as a Tokio task.
///
/// This function returns immediately. The scheduler runs indefinitely in the
/// background until the process exits.
pub fn spawn_scheduler(pool: PgPool) {
    tokio::spawn(async move {
        info!("Scheduler started — tick interval: {}s", TICK_INTERVAL_SECS);

        // Resolve a system user for `updated_by` fields
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

        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(TICK_INTERVAL_SECS));

        loop {
            interval.tick().await;

            // Run both tasks concurrently
            let (reset_result, close_result) = tokio::join!(
                run_menu_auto_reset(&pool, system_user),
                run_session_auto_close(&pool, system_user),
            );

            if let Err(e) = reset_result {
                error!("Scheduler: menu auto-reset failed: {e:#}");
            }
            if let Err(e) = close_result {
                error!("Scheduler: session auto-close failed: {e:#}");
            }
        }
    });
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
}

async fn run_session_auto_close(pool: &PgPool, system_user: Uuid) -> anyhow::Result<()> {
    // Find open sessions whose end_date has passed AND whose restaurant has
    // auto_close_session enabled.
    let candidates = sqlx::query_as!(
        CloseCandidate,
        r#"
        SELECT os.id as session_id,
               os.restaurant_id
        FROM order_sessions os
        JOIN restaurant_order_settings ros ON ros.restaurant_id = os.restaurant_id
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

    for candidate in candidates {
        if let Err(e) = close_session(pool, &candidate, system_user).await {
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

    if result.rows_affected() > 0 {
        info!(
            "Scheduler: auto-closed session {} for restaurant {}",
            candidate.session_id, candidate.restaurant_id
        );
    }

    Ok(())
}