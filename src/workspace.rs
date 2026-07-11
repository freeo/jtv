//! Safe and deterministic discovery of additional workspace Justfiles.

use std::{
    collections::BTreeSet,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkspaceOrigin {
    Root,
    Subfolder {
        relative_justfile: PathBuf,
        label: String,
    },
}

/// A standalone Justfile found below the startup directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceJustfile {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub label: String,
}

/// Non-fatal discovery diagnostics, kept stable for post-TUI reporting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceWarning {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceDiscovery {
    pub justfiles: Vec<WorkspaceJustfile>,
    pub warnings: Vec<WorkspaceWarning>,
}

/// Find independently runnable Justfiles without following links or leaving `root`.
pub fn discover(root: &Path, primary: &Path, module_sources: &[PathBuf]) -> WorkspaceDiscovery {
    let mut result = WorkspaceDiscovery::default();
    let Ok(root) = root.canonicalize() else {
        result.warnings.push(WorkspaceWarning {
            path: root.to_path_buf(),
            message: "cannot canonicalize workspace root".into(),
        });
        return result;
    };
    let primary = primary.canonicalize().ok();
    let excluded_sources: BTreeSet<_> = module_sources
        .iter()
        .filter_map(|path| {
            if path.is_absolute() {
                path.canonicalize().ok()
            } else {
                root.join(path).canonicalize().ok()
            }
        })
        .collect();
    let mut seen = BTreeSet::new();
    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(false)
        .follow_links(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .parents(true)
        .filter_entry(|entry| !is_excluded_dir(entry.path(), entry.file_type()));
    let walker = builder.build();
    for item in walker {
        let entry = match item {
            Ok(entry) => entry,
            Err(error) => {
                result.warnings.push(WorkspaceWarning {
                    path: root.clone(),
                    message: error.to_string(),
                });
                continue;
            }
        };
        if entry.path() == root || !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        if !is_candidate(entry.file_name()) {
            continue;
        }
        let canonical = match entry.path().canonicalize() {
            Ok(path) if path.starts_with(&root) => path,
            Ok(_) => {
                result.warnings.push(WorkspaceWarning {
                    path: entry.path().to_path_buf(),
                    message: "candidate resolves outside workspace".into(),
                });
                continue;
            }
            Err(error) => {
                result.warnings.push(WorkspaceWarning {
                    path: entry.path().to_path_buf(),
                    message: error.to_string(),
                });
                continue;
            }
        };
        if primary.as_ref() == Some(&canonical)
            || excluded_sources.contains(&canonical)
            || !seen.insert(canonical.clone())
        {
            continue;
        }
        let relative = canonical
            .strip_prefix(&root)
            .expect("checked prefix")
            .to_path_buf();
        let Some(relative_text) = relative.to_str() else {
            result.warnings.push(WorkspaceWarning {
                path: relative,
                message: "non-UTF-8 path is not supported".into(),
            });
            continue;
        };
        result.justfiles.push(WorkspaceJustfile {
            path: canonical,
            relative_path: relative.clone(),
            label: source_label(&relative, relative_text),
        });
    }
    result
        .justfiles
        .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    result
        .warnings
        .sort_by(|a, b| a.path.cmp(&b.path).then(a.message.cmp(&b.message)));
    result
}

fn is_candidate(name: &OsStr) -> bool {
    if Path::new(name).extension() == Some(OsStr::new("just")) {
        return true;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    name.eq_ignore_ascii_case("justfile") || name.eq_ignore_ascii_case(".justfile")
}

fn is_excluded_dir(path: &Path, file_type: Option<std::fs::FileType>) -> bool {
    if file_type.is_some_and(|kind| kind.is_symlink()) {
        return true;
    }
    if !file_type.is_some_and(|kind| kind.is_dir()) {
        return false;
    }
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    matches!(name, ".git" | "target" | "node_modules" | ".venv" | "venv")
}

fn source_label(relative: &Path, relative_text: &str) -> String {
    let conventional = relative
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("justfile") || name.eq_ignore_ascii_case(".justfile")
        });
    if conventional {
        let parent = relative.parent().unwrap_or(Path::new(""));
        if parent
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            let text = parent.to_string_lossy().replace('\\', "/");
            return if text.is_empty() {
                "./".into()
            } else {
                format!("{text}/")
            };
        }
    }
    relative_text.replace('\\', "/")
}
