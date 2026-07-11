//! Deterministic resolution of `jtv NAME` convenience targets.

use std::path::{Component, Path, PathBuf};

use crate::{Error, Result, model::Project};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TargetKind {
    Module(String),
    Justfile(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTarget {
    pub name: String,
    pub selected: TargetKind,
    pub matches: Vec<String>,
}

impl ResolvedTarget {
    pub fn warning(&self) -> Option<String> {
        (self.matches.len() > 1).then(|| {
            let targets = self
                .matches
                .iter()
                .map(|target| format!("  {target}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "WARNING: '{}' resolves to multiple targets:\n{targets}",
                self.name
            )
        })
    }
}

/// The only standalone layouts recognized by the positional convenience syntax.
pub fn file_candidates(name: &str) -> Result<Vec<PathBuf>> {
    validate_name(name)?;
    Ok(vec![
        PathBuf::from(format!("{name}.just")),
        PathBuf::from(name).join("justfile"),
        PathBuf::from("justfiles").join(format!("{name}.just")),
        PathBuf::from("justfiles").join(name).join("justfile"),
    ])
}

pub fn resolve(cwd: &Path, name: &str, root_project: Option<&Project>) -> Result<ResolvedTarget> {
    let candidates = file_candidates(name)?;
    let module_exists = root_project.is_some_and(|project| {
        let prefix = format!("{name}::");
        project
            .recipes
            .iter()
            .any(|recipe| recipe.namepath.starts_with(&prefix))
    });

    let mut matches = Vec::new();
    let mut selected = None;
    if module_exists {
        matches.push(format!("module {name}"));
        selected = Some(TargetKind::Module(name.to_owned()));
    }
    for candidate in &candidates {
        if candidate_is_file_without_directory_symlinks(cwd, candidate) {
            matches.push(candidate.display().to_string());
            selected.get_or_insert_with(|| TargetKind::Justfile(candidate.clone()));
        }
    }

    let selected = selected.ok_or_else(|| {
        let searched = candidates
            .iter()
            .map(|path| format!("  {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n");
        Error::Message(format!(
            "target '{name}' was not found; searched:\n{searched}"
        ))
    })?;
    Ok(ResolvedTarget {
        name: name.to_owned(),
        selected,
        matches,
    })
}

fn candidate_is_file_without_directory_symlinks(cwd: &Path, candidate: &Path) -> bool {
    let components = candidate.components().collect::<Vec<_>>();
    let mut path = cwd.to_path_buf();
    for component in &components[..components.len().saturating_sub(1)] {
        path.push(component.as_os_str());
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            return false;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return false;
        }
    }
    cwd.join(candidate).is_file()
}

/// Return whether normal `just` discovery can see a root Justfile.
///
/// `just` recognizes `justfile` and `.justfile` case-insensitively and searches
/// parent directories. This probe only decides whether loading the normal root
/// project should be attempted; `just` remains the parser and authority.
pub fn has_discoverable_root(cwd: &Path) -> bool {
    discoverable_root(cwd).is_some()
}

/// Best-effort path companion to the normal `just` discovery probe.
/// Real `just` JSON supplies its authoritative source; this fallback keeps
/// compatible test doubles and older additive JSON useful.
pub fn discoverable_root(cwd: &Path) -> Option<PathBuf> {
    for directory in cwd.ancestors() {
        let mut candidates = std::fs::read_dir(directory)
            .ok()?
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| {
                let name = entry.file_name().to_str()?.to_ascii_lowercase();
                matches!(name.as_str(), "justfile" | ".justfile")
                    .then_some((name.starts_with('.'), entry.path()))
            })
            .collect::<Vec<_>>();
        candidates.sort();
        if let Some((_, path)) = candidates.into_iter().next() {
            return path.canonicalize().ok();
        }
    }
    None
}

fn validate_name(name: &str) -> Result<()> {
    let mut components = Path::new(name).components();
    let valid = !name.is_empty()
        && !name.contains(['/', '\\'])
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && name != "."
        && name != "..";
    if valid {
        Ok(())
    } else {
        Err(Error::Message(format!(
            "invalid target name '{name}'; expected one non-empty path component"
        )))
    }
}
