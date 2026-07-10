use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Invocation {
    pub cwd: PathBuf,
    pub justfile: Option<PathBuf>,
    pub module_filter: Option<String>,
    pub dry_run: bool,
    pub just_binary: PathBuf,
    pub tv_binary: PathBuf,
}

impl Invocation {
    pub fn new(
        cwd: PathBuf,
        justfile: Option<PathBuf>,
        module_filter: Option<String>,
        dry_run: bool,
    ) -> Self {
        Self {
            cwd,
            justfile,
            module_filter,
            dry_run,
            just_binary: std::env::var_os("JTV_JUST")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("just")),
            tv_binary: std::env::var_os("JTV_TV")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("tv")),
        }
    }

    /// Resolve paths without turning them into shell text.
    pub fn canonicalized(mut self) -> Result<Self> {
        self.cwd = canonicalize(&self.cwd)?;
        if let Some(path) = self.justfile.take() {
            let path = if path.is_absolute() {
                path
            } else {
                self.cwd.join(path)
            };
            self.justfile = Some(canonicalize(&path)?);
        }
        Ok(self)
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf> {
    path.canonicalize().map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })
}
