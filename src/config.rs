use std::sync::OnceLock;

use crate::logging::LoggingConfig;
#[cfg(feature = "smtp")]
use crate::smtp::config::SmtpConfig;

pub struct Config {
    pub logging: LoggingConfig,
    #[cfg(feature = "smtp")]
    pub smtp: SmtpConfig,
}

impl Config {
    pub fn new() -> Self {
        Self {
            logging: LoggingConfig::new(),
            #[cfg(feature = "smtp")]
            smtp: SmtpConfig::new(),
        }
    }

    pub fn get() -> &'static Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        CONFIG.get_or_init(Self::new)
    }
}
