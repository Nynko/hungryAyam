use anyhow::{anyhow, Result};
use lettre::{
    message::header::ContentType,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use tracing::{debug, info};

#[derive(Clone)]
pub struct EmailService {
    from: String,
    host: String,
    port: u16,
    user: String,
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl EmailService {
    pub fn new(host: &str, port: u16, user: &str, password: &str, from: &str) -> Result<Self> {
        let creds = Credentials::new(user.to_string(), password.to_string());

        // Port 465 → implicit SSL; port 587 → STARTTLS
        let transport = if port == 465 {
            info!("EmailService: using implicit SSL (port 465) host={host} user={user}");
            let tls = TlsParameters::new(host.to_owned())
                .map_err(|e| anyhow!("Failed to build TLS params: {e}"))?;
            AsyncSmtpTransport::<Tokio1Executor>::relay(host)?
                .port(port)
                .tls(Tls::Wrapper(tls))
                .credentials(creds)
                .build()
        } else {
            info!("EmailService: using STARTTLS (port {port}) host={host} user={user}");
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
                .port(port)
                .credentials(creds)
                .build()
        };

        Ok(Self {
            from: from.to_string(),
            host: host.to_string(),
            port,
            user: user.to_string(),
            transport,
        })
    }

    pub async fn send(&self, to: &str, subject: &str, body: String) -> Result<()> {
        self.send_inner(to, subject, body, false).await
    }

    pub async fn send_plain(&self, to: &str, subject: &str, body: String) -> Result<()> {
        self.send_inner(to, subject, body, true).await
    }

    async fn send_inner(&self, to: &str, subject: &str, body: String, plain: bool) -> Result<()> {
        info!(
            "EmailService: sending to={to} subject={subject:?} host={} port={} plain={plain}",
            self.host, self.port
        );

        let mut builder = Message::builder()
            .from(self.from.parse().map_err(|e| anyhow!("Invalid from address: {e}"))?)
            .to(to.parse().map_err(|e| anyhow!("Invalid to address: {e}"))?)
            .subject(subject);

        let email = if plain {
            builder
                .header(ContentType::TEXT_PLAIN)
                .body(body)
        } else {
            builder
                .header(ContentType::TEXT_HTML)
                .body(body)
        }
        .map_err(|e| anyhow!("Failed to build email: {e}"))?;

        debug!("EmailService: connecting…");

        match self.transport.send(email).await {
            Ok(response) => {
                info!("EmailService: sent OK — {:?}", response.message().collect::<Vec<_>>());
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    "EmailService: SMTP send failed to={to} host={} port={} — {e}",
                    self.host, self.port
                );
                Err(anyhow!("Failed to send email: {e}"))
            }
        }
    }

    pub async fn send_verification(&self, to: &str, name: &str, token: &str, base_url: &str) -> Result<()> {
        let link = format!("{}/verify-email?token={}", base_url, token);
        let body = format!(
            r#"<p>Hello {name},</p>
<p>Please verify your email address by clicking the link below:</p>
<p><a href="{link}">{link}</a></p>
<p>This link expires in 24 hours.</p>"#
        );
        self.send(to, "Verify your email address", body).await
    }

    pub async fn send_notification(&self, subject: &str, body: String) -> Result<()> {
        self.send(&self.from.clone(), subject, body).await
    }
}
