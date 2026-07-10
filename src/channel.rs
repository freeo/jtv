//! Television cable-channel installation.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use directories::BaseDirs;
use tempfile::NamedTempFile;

use crate::{Error, Result};

pub const CHANNEL_NAME: &str = "jtv-recipes";
pub const CHANNEL_FILE_NAME: &str = "jtv-recipes.toml";
pub const CHANNEL_CONTENT: &str = include_str!("../assets/jtv-recipes.toml");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallOutcome {
    Installed { path: PathBuf },
    AlreadyCurrent { path: PathBuf },
    Replaced { path: PathBuf, backup: PathBuf },
}

pub fn default_cable_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("JTV_TV_CABLE_DIR") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("TELEVISION_CONFIG") {
        return Ok(PathBuf::from(path).join("cable"));
    }
    BaseDirs::new()
        .map(|dirs| dirs.config_dir().join("television").join("cable"))
        .ok_or_else(|| {
            Error::Message("unable to determine Television configuration directory".into())
        })
}

pub fn channel_path(cable_dir: &Path) -> PathBuf {
    cable_dir.join(CHANNEL_FILE_NAME)
}

pub fn channel_is_current(cable_dir: &Path) -> Result<bool> {
    let path = channel_path(cable_dir);
    match fs::read(&path) {
        Ok(bytes) => Ok(bytes == CHANNEL_CONTENT.as_bytes()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::Read { path, source }),
    }
}

pub fn install(cable_dir: &Path, force: bool) -> Result<InstallOutcome> {
    fs::create_dir_all(cable_dir).map_err(|source| Error::Write {
        path: cable_dir.to_path_buf(),
        source,
    })?;
    let path = channel_path(cable_dir);
    if let Ok(existing) = fs::read(&path) {
        if existing == CHANNEL_CONTENT.as_bytes() {
            return Ok(InstallOutcome::AlreadyCurrent { path });
        }
        if !force {
            return Err(Error::Message(format!(
                "Television channel {} already exists and is not managed by this jtv version; rerun `jtv init --force` to back it up and replace it",
                path.display()
            )));
        }
        let backup = next_backup_path(&path);
        fs::copy(&path, &backup).map_err(|source| Error::Write {
            path: backup.clone(),
            source,
        })?;
        atomic_write(&path, CHANNEL_CONTENT.as_bytes())?;
        return Ok(InstallOutcome::Replaced { path, backup });
    } else if let Err(source) = fs::metadata(&path) {
        if source.kind() != std::io::ErrorKind::NotFound {
            return Err(Error::Read { path, source });
        }
    }
    atomic_write(&path, CHANNEL_CONTENT.as_bytes())?;
    Ok(InstallOutcome::Installed { path })
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::Message("channel path has no parent".into()))?;
    let mut temp = NamedTempFile::new_in(parent).map_err(|source| Error::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    temp.write_all(contents)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|source| Error::Write {
            path: path.to_path_buf(),
            source,
        })?;
    temp.persist(path).map_err(|error| Error::Write {
        path: path.to_path_buf(),
        source: error.error,
    })?;
    Ok(())
}

fn next_backup_path(path: &Path) -> PathBuf {
    for index in 0_u32.. {
        let suffix = if index == 0 {
            ".bak".into()
        } else {
            format!(".bak.{index}")
        };
        let candidate = PathBuf::from(format!("{}{}", path.display(), suffix));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}
