use std::error::Error;

use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{RollingFileAppender, Rotation},
};
use tracing_log::LogTracer;
use tracing_subscriber::{Registry, fmt, prelude::*};

use crate::constants::APPLICATION_NAME;

pub fn get_logging_directory() -> Result<String, Box<dyn Error>> {
    let appdata = std::env::var("APPDATA")?;

    Ok(format!("{}/{}/logs", appdata, APPLICATION_NAME))
}

pub fn init_logging() -> Result<WorkerGuard, Box<dyn Error>> {
    LogTracer::init()?;

    let log_dir = get_logging_directory()?;

    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_suffix("log")
        .build(log_dir)?;
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer().with_writer(non_blocking).with_ansi(false);

    let subscriber = cfg_select! {
        debug_assertions => {
            {
                let console_layer = fmt::layer().with_writer(std::io::stdout).with_ansi(false);

                Registry::default().with(file_layer).with(console_layer)
            }
        }
        not(debug_assertions) => {
            Registry::default().with(file_layer)
        }
    };

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(guard)
}
