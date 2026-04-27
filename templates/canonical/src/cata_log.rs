use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::meltdown::*;

#[macro_export]
macro_rules! cata_log {
    (Debug, $msg:expr) => {
        $crate::cata_log::log_debug($msg)
    };
    (Info, $msg:expr) => {
        $crate::cata_log::log_info($msg)
    };
    (Warning, $msg:expr) => {
        $crate::cata_log::log_warn($msg)
    };
    (Error, $msg:expr) => {
        $crate::cata_log::log_error($msg)
    };
    (Trace, $msg:expr) => {
        $crate::cata_log::log_trace($msg)
    };
}

pub fn init_tracing() {
    let is_prod = std::env::var("BUILD_MODE").is_ok_and(|v| v.to_lowercase() == "prod");

    let default_filter = if is_prod {
        "info,tower_http=info,axum::rejection=trace"
    } else {
        "debug,tower_http=debug,axum::rejection=trace"
    };

    let filter = resolve_env_filter(default_filter);

    let registry = tracing_subscriber::registry().with(filter);

    if is_prod {
        registry.with(tracing_subscriber::fmt::layer().json()).init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
}

fn resolve_env_filter(default: &str) -> EnvFilter {
    match try_env_filter() {
        Ok(f) => f,
        Err(e) => {
            cata_log!(Debug, format!("EnvFilter env parse failed, using default: {}", e));
            EnvFilter::new(default)
        }
    }
}

fn try_env_filter() -> Result<EnvFilter, MeltDown> {
    EnvFilter::try_from_default_env().map_err(|e| MeltDown::new(MeltType::ConfigurationError, format!("env filter: {}", e)))
}

#[track_caller]
pub fn log_debug(msg: impl AsRef<str>) {
    let loc = std::panic::Location::caller();
    tracing::debug!(src.file = loc.file(), src.line = loc.line(), "{}", msg.as_ref());
}

#[track_caller]
pub fn log_info(msg: impl AsRef<str>) {
    let loc = std::panic::Location::caller();
    tracing::info!(src.file = loc.file(), src.line = loc.line(), "{}", msg.as_ref());
}

#[track_caller]
pub fn log_warn(msg: impl AsRef<str>) {
    let loc = std::panic::Location::caller();
    tracing::warn!(src.file = loc.file(), src.line = loc.line(), "{}", msg.as_ref());
}

#[track_caller]
pub fn log_error(msg: impl AsRef<str>) {
    let loc = std::panic::Location::caller();
    tracing::error!(src.file = loc.file(), src.line = loc.line(), "{}", msg.as_ref());
}

#[track_caller]
pub fn log_trace(msg: impl AsRef<str>) {
    let loc = std::panic::Location::caller();
    tracing::trace!(src.file = loc.file(), src.line = loc.line(), "{}", msg.as_ref());
}
