//! Private per-launch Television session state.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::{Builder, NamedTempFile};

use crate::{
    Error, Result,
    cleanup::CleanupPath,
    config::Config,
    invocation::Invocation,
    model::{Project, Recipe},
    presentation::PresentationOptions,
    workspace::{WorkspaceOrigin, WorkspaceWarning},
};

pub const SESSION_ENV: &str = "JTV_SESSION";
pub const SESSION_PROTOCOL: u32 = 2;

/// One independently invokable Justfile in the workspace catalog.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogTarget {
    pub origin: WorkspaceOrigin,
    pub invocation: Invocation,
    pub project: Project,
    #[serde(default)]
    pub config: Config,
}

/// Stable, serialization-safe selection identity. Display text is never identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SelectionRef {
    pub target_index: usize,
    pub recipe_namepath: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceCatalog {
    pub root: PathBuf,
    pub targets: Vec<CatalogTarget>,
    pub selections: BTreeMap<String, SelectionRef>,
    #[serde(default)]
    pub warnings: Vec<WorkspaceWarning>,
}

impl WorkspaceCatalog {
    pub fn from_primary(invocation: Invocation, project: Project, config: Config) -> Self {
        let root = invocation.cwd.clone();
        Self::new(
            root,
            vec![CatalogTarget {
                origin: WorkspaceOrigin::Root,
                invocation,
                project,
                config,
            }],
            Vec::new(),
        )
    }

    /// Construct opaque IDs after discovery has established deterministic target order.
    pub fn new(
        root: PathBuf,
        targets: Vec<CatalogTarget>,
        warnings: Vec<WorkspaceWarning>,
    ) -> Self {
        Self {
            root,
            targets,
            selections: BTreeMap::new(),
            warnings,
        }
    }

    pub fn resolve(&self, id: &str) -> Result<ResolvedSelection<'_>> {
        validate_id(id)?;
        let selection = self
            .selections
            .get(id)
            .ok_or_else(|| Error::InvalidSelection(id.into()))?;
        let target = self.targets.get(selection.target_index).ok_or_else(|| {
            Error::InvalidSession(format!(
                "selection `{id}` refers to missing target {}",
                selection.target_index
            ))
        })?;
        let recipe = target
            .project
            .recipe(&selection.recipe_namepath)
            .ok_or_else(|| {
                Error::InvalidSession(format!(
                    "selection `{id}` refers to missing recipe `{}`",
                    selection.recipe_namepath
                ))
            })?;
        Ok(ResolvedSelection {
            selection,
            target,
            recipe,
        })
    }

    fn rekey(&mut self, session_id: &str) -> Result<()> {
        let prefix = session_id.get(..8).ok_or_else(|| {
            Error::InvalidSession("session identifier is too short to key selections".into())
        })?;
        let refs = self
            .targets
            .iter()
            .enumerate()
            .flat_map(|(target_index, target)| {
                target
                    .project
                    .recipes
                    .iter()
                    .map(move |recipe| SelectionRef {
                        target_index,
                        recipe_namepath: recipe.namepath.clone(),
                    })
            })
            .collect::<Vec<_>>();
        if refs.len() > u32::MAX as usize {
            return Err(Error::InvalidSession(
                "workspace contains too many selectable recipes".into(),
            ));
        }
        self.selections = refs
            .into_iter()
            .enumerate()
            .map(|(index, selection)| (format!("jtv-{prefix}-{index:08x}"), selection))
            .collect();
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if !self.root.is_absolute() {
            return Err(Error::InvalidSession(
                "workspace root must be an absolute path".into(),
            ));
        }
        if self.targets.is_empty() {
            return Err(Error::InvalidSession("workspace has no targets".into()));
        }
        let mut seen_pairs = BTreeSet::new();
        for (index, target) in self.targets.iter().enumerate() {
            validate_origin(&target.origin)?;
            if (index == 0) != matches!(target.origin, WorkspaceOrigin::Root) {
                return Err(Error::InvalidSession(
                    "catalog must contain exactly one root target at index zero".into(),
                ));
            }
            if target
                .invocation
                .justfile
                .as_ref()
                .is_some_and(|justfile| !justfile.is_absolute())
            {
                return Err(Error::InvalidSession(format!(
                    "catalog target {index} Justfile path is not absolute"
                )));
            }
            if target.invocation.cwd != self.root {
                return Err(Error::InvalidSession(format!(
                    "catalog target {index} does not preserve the workspace startup directory"
                )));
            }
            if let WorkspaceOrigin::Subfolder {
                relative_justfile, ..
            } = &target.origin
            {
                let justfile = target.invocation.justfile.as_deref().ok_or_else(|| {
                    Error::InvalidSession(format!(
                        "subfolder catalog target {index} has no explicit Justfile"
                    ))
                })?;
                if justfile != self.root.join(relative_justfile) {
                    return Err(Error::InvalidSession(format!(
                        "catalog target {index} origin does not match its Justfile"
                    )));
                }
                if target.config != Config::default() {
                    return Err(Error::InvalidSession(format!(
                        "subfolder target {index} unexpectedly contains recipe configuration"
                    )));
                }
            }
            target
                .config
                .validate(&target.project, Path::new("<session-config>"))
                .map_err(|error| Error::InvalidSession(error.to_string()))?;
            let mut recipe_names = BTreeSet::new();
            for recipe in &target.project.recipes {
                if !recipe_names.insert(&recipe.namepath) {
                    return Err(Error::InvalidSession(format!(
                        "target {index} contains duplicate recipe `{}`",
                        recipe.namepath
                    )));
                }
            }
        }
        for (id, selection) in &self.selections {
            validate_id(id)?;
            if !seen_pairs.insert((selection.target_index, selection.recipe_namepath.as_str())) {
                return Err(Error::InvalidSession(format!(
                    "duplicate selection for target {} recipe `{}`",
                    selection.target_index, selection.recipe_namepath
                )));
            }
            self.resolve(id)?;
        }
        let expected = self
            .targets
            .iter()
            .map(|t| t.project.recipes.len())
            .sum::<usize>();
        if self.selections.len() != expected {
            return Err(Error::InvalidSession(
                "session selections do not cover every catalog recipe".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResolvedSelection<'a> {
    pub selection: &'a SelectionRef,
    pub target: &'a CatalogTarget,
    pub recipe: &'a Recipe,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionState {
    pub protocol: u32,
    pub session_id: String,
    pub catalog: WorkspaceCatalog,
    #[serde(default)]
    pub presentation: PresentationOptions,
}

impl SessionState {
    pub fn new(invocation: Invocation, project: Project) -> Result<Self> {
        Self::new_with_presentation(
            invocation,
            project,
            Config::default(),
            PresentationOptions::default(),
        )
    }

    pub fn new_with_presentation(
        invocation: Invocation,
        project: Project,
        config: Config,
        presentation: PresentationOptions,
    ) -> Result<Self> {
        Self::new_with_catalog(
            WorkspaceCatalog::from_primary(invocation, project, config),
            presentation,
        )
    }

    pub fn new_with_catalog(
        mut catalog: WorkspaceCatalog,
        presentation: PresentationOptions,
    ) -> Result<Self> {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|error| {
            Error::InvalidSession(format!("could not create session ID: {error}"))
        })?;
        let session_id = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        catalog.rekey(&session_id)?;
        catalog.validate()?;
        Ok(Self {
            protocol: SESSION_PROTOCOL,
            session_id,
            catalog,
            presentation,
        })
    }

    pub fn resolve(&self, id: &str) -> Result<ResolvedSelection<'_>> {
        self.catalog.resolve(id)
    }

    pub fn primary_target(&self) -> Result<&CatalogTarget> {
        self.catalog
            .targets
            .first()
            .ok_or_else(|| Error::InvalidSession("workspace has no primary target".into()))
    }

    fn validate(&self) -> Result<()> {
        if self.protocol != SESSION_PROTOCOL {
            return Err(Error::InvalidSession(format!(
                "unsupported protocol {}",
                self.protocol
            )));
        }
        if self.session_id.len() != 32
            || !self
                .session_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(Error::InvalidSession(
                "missing or malformed session identifier".into(),
            ));
        }
        let expected_prefix = format!("jtv-{}-", &self.session_id[..8]);
        if self
            .catalog
            .selections
            .keys()
            .any(|id| !id.starts_with(&expected_prefix))
        {
            return Err(Error::InvalidSession(
                "selection identifier belongs to another session".into(),
            ));
        }
        self.catalog.validate()
    }
}

pub struct SessionFile {
    file: NamedTempFile,
    _cleanup: CleanupPath,
}

impl SessionFile {
    pub fn create(state: &SessionState) -> Result<Self> {
        state.validate()?;
        let mut file = Builder::new()
            .prefix("jtv-session-")
            .tempfile()
            .map_err(|source| Error::Write {
                path: std::env::temp_dir(),
                source,
            })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|source| Error::Write {
                    path: file.path().to_path_buf(),
                    source,
                })?;
        }
        serde_json::to_writer(&mut file, state)?;
        file.flush()
            .and_then(|_| file.as_file().sync_all())
            .map_err(|source| Error::Write {
                path: file.path().to_path_buf(),
                source,
            })?;
        let cleanup = CleanupPath::register(file.path());
        Ok(Self {
            file,
            _cleanup: cleanup,
        })
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }
}

pub fn load(path: &Path) -> Result<SessionState> {
    let bytes = fs::read(path).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let state: SessionState =
        serde_json::from_slice(&bytes).map_err(|error| Error::InvalidSession(error.to_string()))?;
    state.validate()?;
    Ok(state)
}

pub fn load_from_env() -> Result<SessionState> {
    let path = std::env::var_os(SESSION_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| Error::InvalidSession(format!("{SESSION_ENV} is not set")))?;
    load(&path)
}

pub fn validate_id(id: &str) -> Result<()> {
    let valid = id.len() == 21
        && id.starts_with("jtv-")
        && id.as_bytes().get(12) == Some(&b'-')
        && id[4..12]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        && id[13..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidSelection(id.into()))
    }
}

fn validate_origin(origin: &WorkspaceOrigin) -> Result<()> {
    let WorkspaceOrigin::Subfolder {
        relative_justfile,
        label,
    } = origin
    else {
        return Ok(());
    };
    if relative_justfile.as_os_str().is_empty()
        || relative_justfile.is_absolute()
        || relative_justfile
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(Error::InvalidSession(
            "subfolder Justfile path must be a non-empty normalized relative path".into(),
        ));
    }
    if label.is_empty() || label.chars().any(char::is_control) {
        return Err(Error::InvalidSession(
            "subfolder source label is empty or contains control characters".into(),
        ));
    }
    Ok(())
}
