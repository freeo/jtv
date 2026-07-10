//! Private per-launch Television session state.

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::{Builder, NamedTempFile};

use crate::{
    Error, Result, cleanup::CleanupPath, config::Config, invocation::Invocation, model::Project,
    presentation::PresentationOptions,
};

pub const SESSION_ENV: &str = "JTV_SESSION";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionState {
    pub protocol: u32,
    pub session_id: String,
    pub invocation: Invocation,
    pub project: Project,
    pub selections: BTreeMap<String, String>,
    #[serde(default)]
    pub config: Config,
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
        let selections = project
            .recipes
            .iter()
            .enumerate()
            .map(|(index, recipe)| (format!("jtv-{index:08x}"), recipe.namepath.clone()))
            .collect();
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|error| {
            Error::InvalidSession(format!("could not create session ID: {error}"))
        })?;
        Ok(Self {
            protocol: 1,
            session_id: random.iter().map(|byte| format!("{byte:02x}")).collect(),
            invocation,
            project,
            selections,
            config,
            presentation,
        })
    }

    pub fn resolve(&self, id: &str) -> Result<&str> {
        validate_id(id)?;
        self.selections
            .get(id)
            .map(String::as_str)
            .ok_or_else(|| Error::InvalidSelection(id.into()))
    }
}

pub struct SessionFile {
    file: NamedTempFile,
    _cleanup: CleanupPath,
}

impl SessionFile {
    pub fn create(state: &SessionState) -> Result<Self> {
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
    if state.protocol != 1 {
        return Err(Error::InvalidSession(format!(
            "unsupported protocol {}",
            state.protocol
        )));
    }
    if state.session_id.is_empty() {
        return Err(Error::InvalidSession("missing session identifier".into()));
    }
    for id in state.selections.keys() {
        validate_id(id)?;
    }
    Ok(state)
}

pub fn load_from_env() -> Result<SessionState> {
    let path = std::env::var_os(SESSION_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| Error::InvalidSession(format!("{SESSION_ENV} is not set")))?;
    load(&path)
}

pub fn validate_id(id: &str) -> Result<()> {
    let valid = id.len() == 12
        && id.starts_with("jtv-")
        && id[4..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidSelection(id.into()))
    }
}
