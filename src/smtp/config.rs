use std::env;
use std::sync::OnceLock;

pub struct SmtpConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from: String,
    pub smtp_to: String,
    pub smtp_security: SmtpSecurity,
}

impl SmtpConfig {
    pub fn new() -> Self {
        let smtp_security = env::var("SMTP_SECURITY")
            .ok()
            .and_then(|v| SmtpSecurity::from_security_str(&v))
            .unwrap_or(SmtpSecurity::StartTls);

        let smtp_port = env::var("SMTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| smtp_security.default_port());

        Self {
            smtp_host: env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".to_string()),
            smtp_port,
            smtp_username: env::var("SMTP_USERNAME").unwrap_or_default(),
            smtp_password: env::var("SMTP_PASSWORD").unwrap_or_default(),
            smtp_from: env::var("SMTP_FROM")
                .unwrap_or_else(|_| "FAZuH Site <site@fazuh.com>".to_string()),
            smtp_to: env::var("SMTP_TO").unwrap_or_else(|_| "mail@fazuh.com".to_string()),
            smtp_security,
        }
    }

    pub fn get() -> &'static Self {
        static CONFIG: OnceLock<SmtpConfig> = OnceLock::new();
        CONFIG.get_or_init(Self::new)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SmtpSecurity {
    StartTls,
    Tls,
}

impl SmtpSecurity {
    pub fn from_security_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "tls" | "wrapper" | "implicit" => Some(Self::Tls),
            "starttls" | "start_tls" => Some(Self::StartTls),
            _ => None,
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            Self::Tls => 465,
            Self::StartTls => 587,
        }
    }
}
