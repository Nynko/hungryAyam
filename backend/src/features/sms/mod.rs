//! Outbound SMS via the OVHcloud SMS API.
//!
//! Used as a fallback when the iPhone Shortcut (email → SMS) did not confirm
//! delivery in time. Configured with environment variables:
//!
//! - `OVH_APPLICATION_KEY`, `OVH_APPLICATION_SECRET`, `OVH_CONSUMER_KEY`
//!   (create them at https://eu.api.ovh.com/createToken/ with the rights
//!   `POST /sms/*/jobs` and `GET /sms/*`)
//! - `OVH_SMS_SERVICE_NAME` — the SMS account, e.g. `sms-ab12345-1`
//! - `OVH_SMS_SENDER` (optional) — a validated sender name. When unset, OVH
//!   sends from a short number (`senderForResponse`).
//! - `OVH_ENDPOINT` (optional) — defaults to `https://eu.api.ovh.com/1.0`.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::sync::OnceCell;
use tracing::info;

const DEFAULT_ENDPOINT: &str = "https://eu.api.ovh.com/1.0";

#[derive(Clone)]
pub struct OvhSmsClient {
    endpoint: String,
    application_key: String,
    application_secret: String,
    consumer_key: String,
    service_name: String,
    sender: Option<String>,
    http: reqwest::Client,
    /// Offset between OVH server time and local time (seconds), fetched once.
    time_delta: std::sync::Arc<OnceCell<i64>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SmsJobRequest<'a> {
    message: &'a str,
    receivers: [&'a str; 1],
    charset: &'static str,
    coding: &'static str,
    priority: &'static str,
    no_stop_clause: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    sender: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    sender_for_response: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmsJobResponse {
    #[serde(default)]
    pub ids: Vec<i64>,
    #[serde(default)]
    pub valid_receivers: Vec<String>,
    #[serde(default)]
    pub invalid_receivers: Vec<String>,
    #[serde(default)]
    pub total_credits_removed: f64,
}

impl OvhSmsClient {
    pub fn new(
        endpoint: Option<String>,
        application_key: String,
        application_secret: String,
        consumer_key: String,
        service_name: String,
        sender: Option<String>,
    ) -> Self {
        Self {
            endpoint: endpoint
                .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string())
                .trim_end_matches('/')
                .to_string(),
            application_key,
            application_secret,
            consumer_key,
            service_name,
            sender,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("reqwest client"),
            time_delta: Default::default(),
        }
    }

    /// Build a client from `OVH_*` environment variables. Returns `None` if
    /// any required variable is missing or blank.
    pub fn from_env() -> Option<Self> {
        let var = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        Some(Self::new(
            var("OVH_ENDPOINT"),
            var("OVH_APPLICATION_KEY")?,
            var("OVH_APPLICATION_SECRET")?,
            var("OVH_CONSUMER_KEY")?,
            var("OVH_SMS_SERVICE_NAME")?,
            var("OVH_SMS_SENDER"),
        ))
    }

    async fn ovh_timestamp(&self) -> Result<i64> {
        let delta = self
            .time_delta
            .get_or_try_init(|| async {
                let server: i64 = self
                    .http
                    .get(format!("{}/auth/time", self.endpoint))
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                anyhow::Ok(server - chrono::Utc::now().timestamp())
            })
            .await
            .context("failed to fetch OVH server time")?;
        Ok(chrono::Utc::now().timestamp() + delta)
    }

    /// Send one SMS. `to` may be in any format accepted by [`normalize_phone`].
    pub async fn send(&self, to: &str, message: &str) -> Result<SmsJobResponse> {
        let receiver = normalize_phone(to).ok_or_else(|| anyhow!("invalid phone number '{to}'"))?;
        let body = serde_json::to_string(&SmsJobRequest {
            message,
            receivers: [&receiver],
            charset: "UTF-8",
            coding: if is_gsm7(message) { "7bit" } else { "8bit" },
            priority: "high",
            no_stop_clause: true,
            sender: self.sender.as_deref(),
            sender_for_response: self.sender.is_none(),
        })?;

        let url = format!("{}/sms/{}/jobs", self.endpoint, self.service_name);
        let ts = self.ovh_timestamp().await?;
        let signature = sign(&self.application_secret, &self.consumer_key, "POST", &url, &body, ts);

        let res = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Ovh-Application", &self.application_key)
            .header("X-Ovh-Consumer", &self.consumer_key)
            .header("X-Ovh-Timestamp", ts.to_string())
            .header("X-Ovh-Signature", signature)
            .body(body)
            .send()
            .await
            .context("OVH request failed")?;

        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("OVH API returned {status}: {text}"));
        }
        let job: SmsJobResponse =
            serde_json::from_str(&text).with_context(|| format!("unexpected OVH response: {text}"))?;
        if job.valid_receivers.is_empty() {
            return Err(anyhow!("OVH rejected receiver {receiver} (invalid: {:?})", job.invalid_receivers));
        }
        info!(
            "OVH SMS sent to {receiver} (job ids {:?}, {} credit(s))",
            job.ids, job.total_credits_removed
        );
        Ok(job)
    }
}

/// OVH request signature: `"$1$" + sha1_hex(AS+CK+METHOD+URL+BODY+TS joined by '+')`.
pub(crate) fn sign(app_secret: &str, consumer_key: &str, method: &str, url: &str, body: &str, ts: i64) -> String {
    let mut h = Sha1::new();
    h.update(format!("{app_secret}+{consumer_key}+{method}+{url}+{body}+{ts}"));
    format!("$1${}", hex::encode(h.finalize()))
}

/// Normalise a phone number to E.164 (`+33612345678`).
///
/// Accepts spaces, dots, dashes and parentheses as separators, `+` or `00`
/// international prefixes, and French national numbers (`06 12 34 56 78`).
/// Returns `None` if the result doesn't look like a phone number.
pub fn normalize_phone(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let has_plus = trimmed.starts_with('+');
    let mut digits = String::with_capacity(trimmed.len());
    for (i, c) in trimmed.chars().enumerate() {
        match c {
            '0'..='9' => digits.push(c),
            ' ' | '.' | '-' | '(' | ')' => {}
            '+' if i == 0 => {}
            _ => return None,
        }
    }
    let international = if has_plus {
        digits
    } else if let Some(rest) = digits.strip_prefix("00") {
        rest.to_string()
    } else if digits.len() == 10 && digits.starts_with('0') {
        format!("33{}", &digits[1..])
    } else {
        return None;
    };
    // E.164: up to 15 digits, country code never starts with 0
    if !(8..=15).contains(&international.len()) || international.starts_with('0') {
        return None;
    }
    Some(format!("+{international}"))
}

/// True if every character fits the GSM 03.38 7-bit alphabet (incl. extension table).
fn is_gsm7(s: &str) -> bool {
    const GSM7: &str = "@£$¥èéùìòÇ\nØø\rÅåΔ_ΦΓΛΩΠΨΣΘΞÆæßÉ !\"#¤%&'()*+,-./0123456789:;<=>?\
                        ¡ABCDEFGHIJKLMNOPQRSTUVWXYZÄÖÑÜ§¿abcdefghijklmnopqrstuvwxyzäöñüà\
                        ^{}\\[~]|€";
    s.chars().all(|c| GSM7.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_phone_formats() {
        assert_eq!(normalize_phone("+33612345678").as_deref(), Some("+33612345678"));
        assert_eq!(normalize_phone("06 12 34 56 78").as_deref(), Some("+33612345678"));
        assert_eq!(normalize_phone("06.12.34.56.78").as_deref(), Some("+33612345678"));
        assert_eq!(normalize_phone("0033 6 12 34 56 78").as_deref(), Some("+33612345678"));
        assert_eq!(normalize_phone("+65 9123 4567").as_deref(), Some("+6591234567"));
        assert_eq!(normalize_phone("+33 (0)6"), None);
        assert_eq!(normalize_phone("—"), None);
        assert_eq!(normalize_phone(""), None);
        assert_eq!(normalize_phone("612345678"), None);
        assert_eq!(normalize_phone("06-12-34-56-78 ext"), None);
    }

    #[test]
    fn signature_matches_ovh_reference_algorithm() {
        // Reference computed with: printf '%s' 'AS+CK+POST+https://eu.api.ovh.com/1.0/sms/sms-x/jobs+{"a":1}+1700000000' | sha1sum
        assert_eq!(
            sign("AS", "CK", "POST", "https://eu.api.ovh.com/1.0/sms/sms-x/jobs", r#"{"a":1}"#, 1_700_000_000),
            "$1$2c6c26673ecdce8f5c252c2949eb8649e0be5719"
        );
    }

    #[test]
    fn gsm7_detection() {
        assert!(is_gsm7("TIME:12:15\nORDERS:\n2 Ayam Goreng é"));
        assert!(!is_gsm7("Commande 🍗"));
        assert!(!is_gsm7("Œuf"));
    }
}
