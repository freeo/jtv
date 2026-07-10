use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

#[derive(Debug)]
pub struct TestSandbox {
    root: TempDir,
    project: PathBuf,
    home: PathBuf,
    xdg_config: PathBuf,
    xdg_cache: PathBuf,
    xdg_data: PathBuf,
    runtime: PathBuf,
    television: PathBuf,
    cable: PathBuf,
    artifacts: PathBuf,
}

impl TestSandbox {
    pub fn new() -> io::Result<Self> {
        let root = tempfile::Builder::new().prefix("jtv-test-").tempdir()?;
        let join = |name: &str| root.path().join(name);
        let sandbox = Self {
            project: join("project"),
            home: join("home"),
            xdg_config: join("xdg-config"),
            xdg_cache: join("xdg-cache"),
            xdg_data: join("xdg-data"),
            runtime: join("runtime"),
            television: join("xdg-config").join("television"),
            cable: join("xdg-config").join("television").join("cable"),
            artifacts: join("artifacts"),
            root,
        };
        for directory in [
            &sandbox.project,
            &sandbox.home,
            &sandbox.xdg_config,
            &sandbox.xdg_cache,
            &sandbox.xdg_data,
            &sandbox.runtime,
            &sandbox.television,
            &sandbox.cable,
            &sandbox.artifacts,
        ] {
            fs::create_dir_all(directory)?;
        }
        Ok(sandbox)
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }
    pub fn project(&self) -> &Path {
        &self.project
    }
    pub fn home(&self) -> &Path {
        &self.home
    }
    pub fn runtime(&self) -> &Path {
        &self.runtime
    }
    pub fn television(&self) -> &Path {
        &self.television
    }
    pub fn cable(&self) -> &Path {
        &self.cable
    }
    pub fn artifacts(&self) -> &Path {
        &self.artifacts
    }

    pub fn write_project_file(
        &self,
        relative: impl AsRef<Path>,
        bytes: impl AsRef<[u8]>,
    ) -> io::Result<PathBuf> {
        let relative = validate_relative(relative.as_ref())?;
        let path = self.project.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, bytes)?;
        Ok(path)
    }

    pub fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        command.current_dir(&self.project).env_clear();
        for (key, value) in self.environment() {
            command.env(key, value);
        }
        command
    }

    pub fn environment(&self) -> BTreeMap<OsString, OsString> {
        let mut environment = BTreeMap::new();
        environment.insert("HOME".into(), self.home.as_os_str().to_owned());
        environment.insert(
            "XDG_CONFIG_HOME".into(),
            self.xdg_config.as_os_str().to_owned(),
        );
        environment.insert(
            "XDG_CACHE_HOME".into(),
            self.xdg_cache.as_os_str().to_owned(),
        );
        environment.insert("XDG_DATA_HOME".into(), self.xdg_data.as_os_str().to_owned());
        environment.insert(
            "XDG_RUNTIME_DIR".into(),
            self.runtime.as_os_str().to_owned(),
        );
        environment.insert(
            "TELEVISION_CONFIG".into(),
            self.television.as_os_str().to_owned(),
        );
        environment.insert("JTV_TV_CABLE_DIR".into(), self.cable.as_os_str().to_owned());
        environment.insert(
            "JTV_TEST_ARTIFACT_DIR".into(),
            self.artifacts.as_os_str().to_owned(),
        );
        environment.insert("TERM".into(), "xterm-256color".into());
        environment.insert("NO_COLOR".into(), "1".into());
        environment.insert("LANG".into(), "C.UTF-8".into());
        environment.insert("LC_ALL".into(), "C.UTF-8".into());
        environment
    }
}

fn validate_relative(path: &Path) -> io::Result<&Path> {
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "fixture path must stay inside the project",
        ));
    }
    Ok(path)
}
