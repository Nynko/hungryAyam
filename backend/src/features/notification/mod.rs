use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Compute HMAC-SHA256 over `"{ts}|{phone}|{time_str}"`.
/// Returns a lowercase hex string.
pub fn sign(secret: &str, ts: i64, phone: &str, time_str: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(format!("{ts}|{phone}|{time_str}").as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Verify that a signature is correct and that the timestamp is within
/// `max_age_secs` of now (prevents replay of old tokens).
pub fn verify(
    secret: &str,
    sig: &str,
    ts: i64,
    phone: &str,
    time_str: &str,
    max_age_secs: i64,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    if (now - ts).abs() > max_age_secs {
        return Err(anyhow!("token expired (ts={ts}, now={now})"));
    }

    let expected = sign(secret, ts, phone, time_str);

    // Constant-time comparison to avoid timing attacks
    if !constant_time_eq(sig.as_bytes(), expected.as_bytes()) {
        return Err(anyhow!("invalid signature"));
    }

    Ok(())
}

/// Render an SMS message template by substituting placeholders.
///
/// Supported placeholders:
/// - `{PickupTime}` → the pickup time string (e.g. "12:15")
/// - `{Orders}` → the orders summary (e.g. "2 Ayam Goreng\n1 Nasi")
///
/// Falls back to a plain text format when `template` is `None`.
pub fn render_sms_message(template: Option<&str>, pickup_time: &str, orders: &str) -> String {
    match template {
        Some(t) => t
            .replace("{PickupTime}", pickup_time)
            .replace("{Orders}", orders),
        None => format!("TIME:{pickup_time}\nORDERS:\n{orders}"),
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Build and send the order notification email for a session.
///
/// Fetches the order summary from DB, builds the signed body, sends the email,
/// and inserts a `notification_events` row for tracking.
///
/// Callers must already have verified that email is configured and that the
/// session should receive a notification.
pub async fn send_order_notification(
    pool: &PgPool,
    session_id: Uuid,
    pickup_time: Option<chrono::DateTime<chrono::Utc>>,
    restaurant_name: &str,
    restaurant_phone: Option<&str>,
    email_service: &crate::features::email::EmailService,
    notification_email: &str,
    notification_secret: Option<&str>,
    sms_template: Option<&str>,
) -> anyhow::Result<()> {
    // Fetch aggregated order summary for the session
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
        session_id,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let pickup = pickup_time
        .map(|t| t.format("%H:%M").to_string())
        .unwrap_or_else(|| "not set".to_string());

    let phone = restaurant_phone.unwrap_or("—");

    let mut order_lines = String::new();
    for row in &rows {
        order_lines.push_str(&format!("{}x {}\n", row.qty, row.item_name));
    }
    let orders_text = order_lines.trim_end().to_string();

    // Sign if secret is configured
    let (ts, sig_line) = if let Some(secret) = notification_secret {
        let ts = chrono::Utc::now().timestamp();
        let sig = sign(secret, ts, phone, &pickup);
        (ts, format!("TS:{ts}\nSIG:{sig}\n"))
    } else {
        (0, String::new())
    };

    let msg = render_sms_message(sms_template, &pickup, &orders_text);
    let body = format!(
        "{sig_line}RESTAURANT:{restaurant}\nPHONE:{phone}\nTIME:{pickup}\nBODY:{msg}",
        sig_line = sig_line,
        restaurant = restaurant_name,
        phone = phone,
        pickup = pickup,
        msg = msg,
    );

    if let Err(e) = email_service.send_plain(notification_email, "HungryAyam Order", body).await {
        warn!("send_order_notification: failed to send email: {e}");
        return Err(anyhow::anyhow!("Failed to send notification email: {e}"));
    }

    // Insert notification_events row for delivery tracking (only when signed)
    if let Some(secret) = notification_secret {
        let sig = sign(secret, ts, phone, &pickup);
        if let Err(e) = sqlx::query!(
            r#"
            INSERT INTO notification_events
                (sig, session_id, ts, phone, time_str, orders, restaurant)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (sig) DO NOTHING
            "#,
            sig,
            session_id,
            ts,
            phone,
            pickup,
            orders_text,
            restaurant_name,
        )
        .execute(pool)
        .await
        {
            warn!("send_order_notification: failed to insert notification_event: {e}");
        }
    }

    Ok(())
}

pub mod routes;
