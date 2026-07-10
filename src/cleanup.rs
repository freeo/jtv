//! Process-wide cleanup registry used by the Ctrl-C/SIGTERM handler.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use crate::{Error, Result};

static PATHS: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();
static INSTALLED: OnceLock<()> = OnceLock::new();

pub fn install_signal_handler() -> Result<()> {
    if INSTALLED.get().is_some() {
        return Ok(());
    }
    ctrlc::set_handler(|| {
        if let Some(paths) = PATHS.get() {
            if let Ok(mut paths) = paths.lock() {
                for path in std::mem::take(&mut *paths) {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        std::process::exit(130);
    })
    .map_err(|error| Error::Message(format!("unable to install signal handler: {error}")))?;
    let _ = INSTALLED.set(());
    Ok(())
}

pub struct CleanupPath {
    path: PathBuf,
}

impl CleanupPath {
    pub fn register(path: &Path) -> Self {
        let path = path.to_path_buf();
        if let Ok(mut paths) = PATHS.get_or_init(|| Mutex::new(BTreeSet::new())).lock() {
            paths.insert(path.clone());
        }
        Self { path }
    }
}

impl Drop for CleanupPath {
    fn drop(&mut self) {
        if let Some(paths) = PATHS.get() {
            if let Ok(mut paths) = paths.lock() {
                paths.remove(&self.path);
            }
        }
    }
}
