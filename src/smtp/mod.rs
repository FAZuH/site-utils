pub mod config;

use lettre::AsyncSmtpTransport;
use lettre::AsyncTransport;
use lettre::Message;
use lettre::Tokio1Executor;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::Tls;
use lettre::transport::smtp::client::TlsParameters;
use tracing::error;

use crate::smtp::config::SmtpConfig;
use crate::smtp::config::SmtpSecurity;

pub struct EmailData {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub subject: Option<String>,
    pub message: String,
}

/// Build a `lettre::Message` from form data and config.
pub fn build_message(data: &EmailData, config: &SmtpConfig) -> Result<Message, String> {
    let from: Mailbox = config
        .smtp_from
        .parse()
        .map_err(|e| format!("Invalid SMTP_FROM address: {e}"))?;
    let to: Mailbox = config
        .smtp_to
        .parse()
        .map_err(|e| format!("Invalid SMTP_TO address: {e}"))?;

    let reply_to = match &data.email {
        Some(email) => format!("{} <{}>", data.name, email)
            .parse()
            .map_err(|e| format!("Invalid reply-to address: {e}"))?,
        None => from.clone(),
    };

    let subject = data
        .subject
        .clone()
        .unwrap_or_else(|| format!("[fazuh-site] Message from {}", data.name));

    let email_display = data.email.as_deref().unwrap_or("no-reply");

    Message::builder()
        .from(from)
        .to(to)
        .reply_to(reply_to)
        .subject(subject)
        .header(lettre::message::header::ContentType::TEXT_PLAIN)
        .body(format!(
            "From: {name} <{email}>\nPhone: {phone}\n\n{message}",
            name = data.name,
            email = email_display,
            phone = data.phone.as_deref().unwrap_or("-"),
            message = data.message,
        ))
        .map_err(|e| format!("Failed to build email: {e}"))
}

/// Build an SMTP transport from config.
pub fn build_transport(config: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
    Ok(match config.smtp_security {
        SmtpSecurity::StartTls => {
            if config.smtp_password.is_empty() {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
                    .map_err(|e| format!("Failed to configure SMTP relay: {e}"))?
                    .port(config.smtp_port)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
                    .map_err(|e| format!("Failed to configure SMTP relay: {e}"))?
                    .port(config.smtp_port)
                    .credentials(Credentials::new(
                        config.smtp_username.clone(),
                        config.smtp_password.clone(),
                    ))
                    .build()
            }
        }
        SmtpSecurity::Tls => {
            let tls_params = TlsParameters::builder(config.smtp_host.clone())
                .build()
                .map_err(|e| format!("Failed to build TLS parameters: {e}"))?;

            if config.smtp_password.is_empty() {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
                    .map_err(|e| format!("Failed to configure SMTP relay: {e}"))?
                    .port(config.smtp_port)
                    .tls(Tls::Wrapper(tls_params))
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
                    .map_err(|e| format!("Failed to configure SMTP relay: {e}"))?
                    .port(config.smtp_port)
                    .tls(Tls::Wrapper(tls_params))
                    .credentials(Credentials::new(
                        config.smtp_username.clone(),
                        config.smtp_password.clone(),
                    ))
                    .build()
            }
        }
        SmtpSecurity::Plain => {
            // Opportunistic STARTTLS: upgrade if available, otherwise stay plain.
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
                .map_err(|e| format!("Failed to configure SMTP relay: {e}"))?
                .port(config.smtp_port)
                .tls(Tls::Opportunistic(
                    TlsParameters::builder(config.smtp_host.clone())
                        .build()
                        .map_err(|e| format!("Failed to build TLS parameters: {e}"))?,
                ))
                .build()
        }
    })
}

/// Send an email, reading config from environment variables.
pub async fn send_email(form: impl Into<EmailData>) -> Result<(), String> {
    send_email_with_config(form, SmtpConfig::get()).await
}

/// Send an email with an explicit config (useful for tests).
pub async fn send_email_with_config(
    form: impl Into<EmailData>,
    config: &SmtpConfig,
) -> Result<(), String> {
    let data = form.into();
    let email = build_message(&data, config)?;
    let transport = build_transport(config)?;

    transport.send(email).await.map(|_| ()).map_err(|e| {
        error!("Failed to send email via SMTP: {e}");
        format!("SMTP send failed: {e}")
    })
}

#[cfg(test)]
mod tests {
    use lettre::Transport;
    use lettre::transport::stub::StubTransport;

    use super::*;

    fn base_config() -> SmtpConfig {
        SmtpConfig {
            smtp_host: "smtp.example.com".into(),
            smtp_port: 587,
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_from: "Site <site@example.com>".into(),
            smtp_to: "Owner <owner@example.com>".into(),
            smtp_security: SmtpSecurity::StartTls,
        }
    }

    fn base_data() -> EmailData {
        EmailData {
            name: "Alice".into(),
            email: Some("alice@test.com".into()),
            phone: None,
            subject: Some("Hello".into()),
            message: "I would like to discuss a project.".into(),
        }
    }

    /// Send a Message through StubTransport and return the raw MIME string.
    fn capture(msg: &Message) -> String {
        let transport = StubTransport::new_ok();
        transport.send(msg).unwrap();
        transport.messages().into_iter().next().unwrap().1
    }

    // ── build_message: reply-to ─────────────────────────────────────────

    #[test]
    fn reply_to_uses_email_when_present() {
        let msg = build_message(&base_data(), &base_config()).unwrap();
        let raw = capture(&msg);

        assert!(
            raw.contains("Reply-To: Alice <alice@test.com>"),
            "reply-to should be Name <email> when email is Some: {raw}"
        );
    }

    #[test]
    fn reply_to_falls_back_to_from_when_email_none() {
        let data = EmailData {
            email: None,
            ..base_data()
        };
        let msg = build_message(&data, &base_config()).unwrap();
        let raw = capture(&msg);

        assert!(
            raw.contains("Reply-To: Site <site@example.com>"),
            "reply-to should match From when email is None: {raw}"
        );
    }

    // ── build_message: subject ──────────────────────────────────────────

    #[test]
    fn subject_uses_data_when_present() {
        let msg = build_message(&base_data(), &base_config()).unwrap();
        let raw = capture(&msg);

        assert!(raw.contains("Subject: Hello"));
    }

    #[test]
    fn subject_defaults_when_none() {
        let data = EmailData {
            subject: None,
            ..base_data()
        };
        let msg = build_message(&data, &base_config()).unwrap();
        let raw = capture(&msg);

        assert!(
            raw.contains("Subject: [fazuh-site] Message from Alice"),
            "subject should default to '[fazuh-site] Message from {{name}}': {raw}"
        );
    }

    // ── build_message: body ─────────────────────────────────────────────

    #[test]
    fn includes_phone_in_body_when_present() {
        let data = EmailData {
            phone: Some("08123456789".into()),
            ..base_data()
        };
        let msg = build_message(&data, &base_config()).unwrap();
        let raw = capture(&msg);

        assert!(raw.contains("Phone: 08123456789"));
    }

    #[test]
    fn shows_dash_for_missing_phone() {
        let msg = build_message(&base_data(), &base_config()).unwrap();
        let raw = capture(&msg);

        assert!(raw.contains("Phone: -"));
    }

    // ── build_message: from / to ────────────────────────────────────────

    #[test]
    fn from_and_to_match_config() {
        let msg = build_message(&base_data(), &base_config()).unwrap();
        let raw = capture(&msg);

        assert!(raw.contains("From: Site <site@example.com>"));
        assert!(raw.contains("To: Owner <owner@example.com>"));
    }

    // ── build_message: content-type ─────────────────────────────────────

    #[test]
    fn content_type_is_text_plain() {
        let msg = build_message(&base_data(), &base_config()).unwrap();
        let raw = capture(&msg);

        assert!(raw.contains("Content-Type: text/plain"));
    }

    // ── build_message: error paths ──────────────────────────────────────

    #[test]
    fn rejects_invalid_from_address() {
        let mut config = base_config();
        config.smtp_from = "not-an-address".into();

        let result = build_message(&base_data(), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SMTP_FROM"));
    }

    #[test]
    fn rejects_invalid_to_address() {
        let mut config = base_config();
        config.smtp_to = "also-not-valid".into();

        let result = build_message(&base_data(), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SMTP_TO"));
    }

    #[test]
    fn rejects_invalid_reply_to_when_name_contains_brackets() {
        let data = EmailData {
            name: "Alice <hacker>".into(),
            ..base_data()
        };
        let result = build_message(&data, &base_config());
        assert!(result.is_err(), "bracket in name should make reply-to fail");
        assert!(result.unwrap_err().contains("reply-to"));
    }
}
