use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static LOGGER: OnceLock<LoggerState> = OnceLock::new();

struct LoggerState {
    enabled: bool,
    file: Option<Mutex<File>>,
}

pub fn init(argv0: &str) -> Result<()> {
    let enabled = env_flag_enabled("AGT_LOG");
    let file = if enabled {
        let path = resolve_log_path(argv0)?;
        let f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| {
                format!(
                    "AGT_LOG requested but cannot write log file at {}",
                    path.display()
                )
            })?;
        Some(Mutex::new(f))
    } else {
        None
    };

    let _ = LOGGER.set(LoggerState { enabled, file });

    if enabled {
        debug_log("logging initialized");
    }

    Ok(())
}

pub fn is_enabled() -> bool {
    LOGGER.get().map(|state| state.enabled).unwrap_or(false)
}

pub fn debug_log(message: &str) {
    let Some(state) = LOGGER.get() else {
        return;
    };
    if !state.enabled {
        return;
    }
    let Some(mutex) = &state.file else {
        return;
    };
    if let Ok(mut file) = mutex.lock() {
        let _ = writeln!(file, "[agt] {message}");
    }
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !(value.is_empty() || value == "0" || value == "false" || value == "no")
        })
        .unwrap_or(false)
}

fn resolve_log_path(argv0: &str) -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("AGT_LOG_PATH") {
        return Ok(PathBuf::from(path));
    }

    let script_name = Path::new(argv0)
        .file_name()
        .unwrap_or_else(|| OsStr::new("agt"));
    let log_name = format!("{}.log", script_name.to_string_lossy());
    Ok(std::env::current_dir()?.join(log_name))
}
