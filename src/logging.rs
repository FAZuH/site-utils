use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub struct LoggingConfig {
    pub log_level: String,
    pub default_log_dir: String,
    pub log_filename: String,
}

impl LoggingConfig {
    pub fn new() -> Self {
        Self {
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "INFO".to_string()),
            default_log_dir: env::var("DEFAULT_LOG_DIR").unwrap_or_else(|_| "./logs".to_string()),
            log_filename: env::var("LOG_FILENAME").unwrap_or_else(|_| "app.log".to_string()),
        }
    }
    pub fn get() -> &'static Self {
        static CONFIG: OnceLock<LoggingConfig> = OnceLock::new();
        CONFIG.get_or_init(Self::new)
    }
}

pub fn init_logging() {
    let config = LoggingConfig::get();

    let log_dir = PathBuf::from(&config.default_log_dir);
    let file_appender = tracing_appender::rolling::daily(log_dir, &config.log_filename);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_target(false)
        .with_ansi(false)
        .with_timer(tracing_subscriber::fmt::time::time())
        .with_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        );

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(true)
        .with_timer(tracing_subscriber::fmt::time::time())
        .with_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        );

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .init();
}
