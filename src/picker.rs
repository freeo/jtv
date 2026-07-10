//! Nested Television pickers for enumerable values.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use tempfile::Builder;
use walkdir::WalkDir;

use crate::{Error, Result, cleanup::CleanupPath, television::sanitize_field};

pub const PICKER_ENV: &str = "JTV_PICKER_STATE";
pub const PICKER_BIN_ENV: &str = "JTV_BIN";
const PICKER_SOURCE_COMMAND: &str = "\"$JTV_BIN\" __picker-source";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    File,
    Directory,
}

/// Collection selection boundary. The production implementation may use TV;
/// tests can supply a deterministic implementation without a terminal.
pub trait Picker {
    fn choose(&mut self, label: &str, values: &[String]) -> Result<Option<String>>;
    fn choose_path(&mut self, label: &str, root: &Path, kind: PathKind) -> Result<Option<PathBuf>>;
}

#[derive(Clone, Debug)]
pub struct TvPicker {
    tv_binary: PathBuf,
    jtv_binary: PathBuf,
}

impl TvPicker {
    pub fn new(tv_binary: PathBuf) -> Result<Self> {
        let jtv_binary = std::env::current_exe().map_err(|source| Error::Read {
            path: PathBuf::from("/proc/self/exe"),
            source,
        })?;
        Ok(Self {
            tv_binary,
            jtv_binary,
        })
    }

    fn select(
        &self,
        label: &str,
        root: &Path,
        entries: Vec<PickerEntry>,
    ) -> Result<Option<PickerEntry>> {
        let state = PickerState { entries };
        let mut file = Builder::new()
            .prefix("jtv-picker-")
            .tempfile()
            .map_err(|source| Error::Write {
                path: std::env::temp_dir(),
                source,
            })?;
        let _cleanup = CleanupPath::register(file.path());
        serde_json::to_writer(&mut file, &state)?;
        file.flush().map_err(|source| Error::Write {
            path: file.path().to_path_buf(),
            source,
        })?;

        let child = Command::new(&self.tv_binary)
            .arg("--source-command")
            .arg(PICKER_SOURCE_COMMAND)
            .arg("--source-display")
            .arg("{split:\t:1}")
            .arg("--source-output")
            .arg("{split:\t:0}")
            .arg("--input-header")
            .arg(label)
            .arg("--no-remote")
            .current_dir(root)
            .env(PICKER_ENV, file.path())
            .env(PICKER_BIN_ENV, &self.jtv_binary)
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|source| Error::Spawn {
                program: self.tv_binary.display().to_string(),
                source,
            })?;
        let output = child.wait_with_output().map_err(|source| Error::Spawn {
            program: self.tv_binary.display().to_string(),
            source,
        })?;
        if !output.status.success() {
            return Err(Error::ProgramFailed {
                program: self.tv_binary.display().to_string(),
                status: output.status.code().unwrap_or(130),
                stderr: String::new(),
            });
        }
        let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if id.is_empty() {
            return Ok(None);
        }
        state
            .entries
            .into_iter()
            .find(|entry| entry.id == id)
            .map(Some)
            .ok_or(Error::InvalidSelection(id))
    }
}

impl Picker for TvPicker {
    fn choose(&mut self, label: &str, values: &[String]) -> Result<Option<String>> {
        let entries = values
            .iter()
            .enumerate()
            .map(|(index, value)| PickerEntry {
                id: picker_id(index),
                display: value.clone(),
                value: value.clone(),
            })
            .collect();
        Ok(self
            .select(label, Path::new("."), entries)?
            .map(|entry| entry.value))
    }

    fn choose_path(&mut self, label: &str, root: &Path, kind: PathKind) -> Result<Option<PathBuf>> {
        let entries = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.path() != root)
            .filter(|entry| match kind {
                PathKind::File => entry.file_type().is_file(),
                PathKind::Directory => entry.file_type().is_dir(),
            })
            .filter_map(|entry| {
                let relative = entry.path().strip_prefix(root).ok()?;
                let value = relative.to_str()?.to_owned();
                Some(value)
            })
            .enumerate()
            .map(|(index, value)| PickerEntry {
                id: picker_id(index),
                display: value.clone(),
                value,
            })
            .collect();
        Ok(self
            .select(label, root, entries)?
            .map(|entry| root.join(entry.value)))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PickerState {
    entries: Vec<PickerEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PickerEntry {
    id: String,
    display: String,
    value: String,
}

impl PickerState {
    pub fn load_from_env() -> Result<Self> {
        let path = std::env::var_os(PICKER_ENV)
            .map(PathBuf::from)
            .ok_or_else(|| Error::InvalidSession(format!("{PICKER_ENV} is not set")))?;
        let bytes = fs::read(&path).map_err(|source| Error::Read { path, source })?;
        serde_json::from_slice(&bytes).map_err(Error::from)
    }

    pub fn source_output(&self) -> String {
        self.entries
            .iter()
            .map(|entry| format!("{}\t{}", entry.id, sanitize_field(&entry.display)))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn picker_id(index: usize) -> String {
    format!("pick-{index:08x}")
}
