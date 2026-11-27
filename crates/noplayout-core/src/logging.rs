//! Logging: stderr plus daily rolling files in `<data>/logs/` (IDEA §6: tracing +
//! tracing-appender).

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

/// Files kept before the oldest is deleted.
pub const MAX_LOG_FILES: usize = 14;

/// Install the global subscriber. Keep the returned guard alive for the life of the process,
/// or buffered lines are lost at exit. `RUST_LOG` overrides the default `info` level.
pub fn init(log_dir: &Path, file_prefix: &str) -> anyhow::Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(file_prefix)
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(log_dir)?;
    let (file_writer, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(fmt::layer().with_ansi(false).with_writer(file_writer))
        .try_init()?;
    Ok(guard)
}
