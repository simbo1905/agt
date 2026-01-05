use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum IsolationMode {
    None,
    Xdg,
    Chroot,
}

impl std::str::FromStr for IsolationMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "none" => Ok(Self::None),
            "xdg" => Ok(Self::Xdg),
            "chroot" => Ok(Self::Chroot),
            _ => anyhow::bail!("Invalid isolation mode: {}", s),
        }
    }
}

pub struct SessionPaths {
    pub root: PathBuf,
    pub sandbox: PathBuf,
    pub xdg_data: PathBuf,
    pub xdg_config: PathBuf,
}

impl SessionPaths {
    pub fn new(session_root: PathBuf) -> Self {
        Self {
            sandbox: session_root.join("sandbox"),
            xdg_data: session_root.join("xdg"),
            xdg_config: session_root.join("config"),
            root: session_root,
        }
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.sandbox).context("Failed to create sandbox dir")?;
        std::fs::create_dir_all(&self.xdg_data).context("Failed to create xdg data dir")?;
        std::fs::create_dir_all(&self.xdg_config).context("Failed to create xdg config dir")?;
        Ok(())
    }
}
