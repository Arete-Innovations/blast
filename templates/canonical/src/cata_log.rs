use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::meltdown::*;

#[macro_export]
macro_rules! cata_log {
    (Debug, $msg:expr) => {
        ::tracing::debug!("{}", $msg)
    };
    (Info, $msg:expr) => {
        ::tracing::info!("{}", $msg)
    };
    (Warning, $msg:expr) => {
        ::tracing::warn!("{}", $msg)
    };
    (Error, $msg:expr) => {
        ::tracing::error!("{}", $msg)
    };
    (Trace, $msg:expr) => {
        ::tracing::trace!("{}", $msg)
    };
}

pub fn init_tracing() {
    let is_prod = cfg!(feature = "prod");

    let default_filter = if is_prod {
        "info,tower_http=info,axum::rejection=trace"
    } else {
        "debug,tower_http=debug,axum::rejection=trace"
    };

    let filter = resolve_env_filter(default_filter);
    let registry = tracing_subscriber::registry().with(filter);

    if is_prod {
        registry.with(tracing_subscriber::fmt::layer().json().with_file(true).with_line_number(true)).init();
    } else {
        registry.with(tracing_subscriber::fmt::layer().compact().with_target(false).with_file(true).with_line_number(true)).init();
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
