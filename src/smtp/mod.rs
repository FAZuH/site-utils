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
    pub email: String,
    pub subject: Option<String>,
    pub message: String,
}

pub async fn send_email(form: impl Into<EmailData>) -> Result<(), String> {
    let form = form.into();
    let config = SmtpConfig::get();

    let from: Mailbox = config
        .smtp_from
        .parse()
        .map_err(|e| format!("Invalid SMTP_FROM address: {e}"))?;
    let to: Mailbox = config
        .smtp_to
        .parse()
        .map_err(|e| format!("Invalid SMTP_TO address: {e}"))?;

    let reply_to = format!("{} <{}>", form.name, form.email)
        .parse()
        .map_err(|e| format!("Invalid reply-to address: {e}"))?;

    let subject = form
        .subject
        .unwrap_or_else(|| format!("[fazuh-site] Message from {}", form.name));

    let email = Message::builder()
        .from(from)
        .to(to)
        .reply_to(reply_to)
        .subject(subject)
        .header(lettre::message::header::ContentType::TEXT_PLAIN)
        .body(format!(
            "From: {name} <{email}>\n\n{message}",
            name = form.name,
            email = form.email,
            message = form.message,
        ))
        .map_err(|e| format!("Failed to build email: {e}"))?;

    let transport = if config.smtp_security == SmtpSecurity::Tls {
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
    } else {
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
    };

    match transport.send(email).await {
        Ok(_response) => Ok(()),
        Err(e) => {
            error!("Failed to send email via SMTP: {e}");
            Err(format!("SMTP send failed: {e}"))
        }
    }
}
