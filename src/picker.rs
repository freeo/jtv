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
    /// Complete an ordinary string with a file or directory below `root`.
    /// The returned path is relative to `root`.
    fn complete_path(
        &mut self,
        label: &str,
        root: &Path,
        initial_query: &str,
    ) -> Result<Option<PathBuf>> {
        let _ = (label, root, initial_query);
        Err(Error::Message(
            "this picker does not support path completion".into(),
        ))
    }
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
        initial_query: Option<&str>,
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

        let mut command = Command::new(&self.tv_binary);
        command
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
            .stderr(Stdio::inherit());
        if let Some(query) = initial_query {
            command.arg("--input").arg(query);
        }
        let child = command.spawn().map_err(|source| Error::Spawn {
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
            .select(label, Path::new("."), entries, None)?
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
            .select(label, root, entries, None)?
            .map(|entry| root.join(entry.value)))
    }

    fn complete_path(
        &mut self,
        label: &str,
        root: &Path,
        initial_query: &str,
    ) -> Result<Option<PathBuf>> {
        let values = recursive_relative_paths(root);
        let entries = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| PickerEntry {
                id: picker_id(index),
                display: value.clone(),
                value,
            })
            .collect();
        Ok(self
            .select(label, root, entries, Some(initial_query))?
            .map(|entry| PathBuf::from(entry.value)))
    }
}

fn recursive_relative_paths(root: &Path) -> Vec<String> {
    let mut values: Vec<_> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path() != root)
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(root)
                .ok()?
                .to_str()
                .map(str::to_owned)
        })
        .collect();
    values.sort();
    values.dedup();
    values
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_paths_are_recursive_relative_hidden_and_sorted() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("z/nested")).unwrap();
        fs::create_dir(root.path().join(".hidden")).unwrap();
        fs::write(root.path().join("z/nested/file name"), "").unwrap();
        fs::write(root.path().join("a"), "").unwrap();
        assert_eq!(
            recursive_relative_paths(root.path()),
            [".hidden", "a", "z", "z/nested", "z/nested/file name"]
        );
    }

    #[test]
    fn completion_handles_an_empty_and_a_larger_tree_without_a_hidden_cap() {
        let empty = tempfile::tempdir().unwrap();
        assert!(recursive_relative_paths(empty.path()).is_empty());

        let larger = tempfile::tempdir().unwrap();
        for index in 0..512 {
            fs::write(larger.path().join(format!("entry-{index:04}")), "").unwrap();
        }
        assert_eq!(recursive_relative_paths(larger.path()).len(), 512);
    }

    #[cfg(unix)]
    #[test]
    fn completion_omits_non_utf8_paths_that_television_cannot_display() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join(OsString::from_vec(vec![b'f', 0xff])), "").unwrap();
        assert!(recursive_relative_paths(root.path()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn completion_lists_but_does_not_follow_directory_symlinks() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("real/child")).unwrap();
        symlink(root.path().join("real"), root.path().join("linked")).unwrap();
        let paths = recursive_relative_paths(root.path());
        assert!(paths.iter().any(|path| path == "linked"));
        assert!(!paths.iter().any(|path| path == "linked/child"));
    }
}
