use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;

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

pub mod routes;
