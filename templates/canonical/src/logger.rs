
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_tracing() {
    let is_prod = std::env::var("BUILD_MODE")
        .map(|v| v.to_lowercase() == "prod")
        .unwrap_or(false);

    let default_filter = if is_prod {
        "info,tower_http=info,axum::rejection=trace"
    } else {
        "debug,tower_http=debug,axum::rejection=trace"
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_filter));

    let registry = tracing_subscriber::registry().with(filter);

    if is_prod {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
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

#[macro_export]
macro_rules! cata_log {
    (Debug, $msg:expr) => {
        $crate::logger::log_debug($msg)
    };
    (Info, $msg:expr) => {
        $crate::logger::log_info($msg)
    };
    (Warning, $msg:expr) => {
        $crate::logger::log_warn($msg)
    };
    (Error, $msg:expr) => {
        $crate::logger::log_error($msg)
    };
    (Trace, $msg:expr) => {
        $crate::logger::log_trace($msg)
    };
}
