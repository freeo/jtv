use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use std::time::Instant;

use portable_pty::ExitStatus;

use super::artifacts::{FailureArtifacts, SemanticEvent};
use super::keys::Key;
use super::pty::{PtyCommand, PtySession};
use super::sandbox::TestSandbox;
use super::screen::ScreenFrame;

pub const SCREEN_TIMEOUT: Duration = Duration::from_secs(15);
pub const EXIT_TIMEOUT: Duration = Duration::from_secs(15);

pub struct RealTvScenario {
    pub sandbox: TestSandbox,
    pub event_log: PathBuf,
    pub session: PtySession,
    artifacts: FailureArtifacts,
    artifacts_persisted: bool,
}

impl RealTvScenario {
    pub fn launch(name: &str) -> io::Result<Self> {
        Self::launch_with_viewport(name, 160, 40)
    }

    /// Launch a real-TV scenario at an explicit character-cell viewport.
    ///
    /// Workflow tests retain their wider historical viewport, while reviewed
    /// Linux snapshots use the canonical 120x40 contract.
    pub fn launch_with_viewport(name: &str, columns: u16, rows: u16) -> io::Result<Self> {
        require_real_tools()?;
        let sandbox = TestSandbox::new()?;
        copy_fixture(&sandbox)?;
        let event_log = sandbox.root().join("executions.log");
        fs::write(&event_log, [])?;
        install_channel(&sandbox, &event_log)?;
        let command = jtv_command(&sandbox, &event_log, columns, rows);
        let session = PtySession::spawn(command)?;
        let artifacts = FailureArtifacts::new(
            name,
            Path::new(env!("CARGO_MANIFEST_DIR")),
            sandbox.root(),
            true,
        )?;
        Ok(Self {
            sandbox,
            event_log,
            session,
            artifacts,
            artifacts_persisted: false,
        })
    }

    pub fn add_secret(&mut self, value: &str) {
        self.artifacts.sanitizer_mut().add_secret(value);
    }

    pub fn wait(&mut self, description: &str, needle: &str) -> ScreenFrame {
        match self
            .session
            .wait_for_screen(description, SCREEN_TIMEOUT, |frame| frame.contains(needle))
        {
            Ok(frame) => {
                self.artifacts.push_event(SemanticEvent::ScreenCondition {
                    description: description.into(),
                });
                // A semantic condition proves the UI arrived; a short quiet
                // period then avoids typing into a prompt while it is still
                // switching terminal modes.
                self.session
                    .wait_for_quiet(Duration::from_millis(40), Duration::from_secs(2))
                    .unwrap_or(frame)
            }
            Err(error) => self.fail(&error.to_string()),
        }
    }

    pub fn text(&mut self, value: &str) {
        self.session
            .send_text(value)
            .unwrap_or_else(|error| self.fail(&error.to_string()));
        self.artifacts.push_event(SemanticEvent::Text {
            value: value.into(),
        });
    }

    pub fn secret(&mut self, value: &str) {
        self.session
            .send_secret(value)
            .unwrap_or_else(|error| self.fail(&error.to_string()));
        self.artifacts.push_event(SemanticEvent::SecretInput);
    }

    pub fn key(&mut self, key: Key) {
        self.session
            .send_key(key)
            .unwrap_or_else(|error| self.fail(&error.to_string()));
        self.artifacts
            .push_event(SemanticEvent::Key { name: key.name() });
    }

    pub fn select_recipe(&mut self, query: &str) -> ScreenFrame {
        self.wait("recipe browser", "jtv-recipes");
        self.text(query);
        self.wait("filtered recipe and preview", query)
    }

    pub fn confirm(&mut self) {
        self.wait("confirmation prompt", "Run selected recipe(s)?");
        self.key(Key::Enter);
    }

    pub fn exit(&mut self) -> ExitStatus {
        let group = self.session.process_group_leader();
        match self.session.wait_for_exit(EXIT_TIMEOUT) {
            Ok(status) => {
                if let Some(group) = group {
                    wait_for_process_group_exit(group);
                }
                self.artifacts.push_event(SemanticEvent::Exit {
                    status: Some(status.exit_code() as i32),
                });
                status
            }
            Err(error) => self.fail(&error.to_string()),
        }
    }

    pub fn events(&self) -> Vec<String> {
        fs::read_to_string(&self.event_log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    pub fn assert_clean(&self) {
        let residue = fs::read_dir(self.sandbox.runtime())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("jtv-session-") || name.starts_with("jtv-picker-"))
            .collect::<Vec<_>>();
        assert!(residue.is_empty(), "temporary state remains: {residue:?}");
        assert!(!self.sandbox.project().join(".just_history").exists());
        assert!(
            !self
                .sandbox
                .project()
                .join(".just-tv-last-command")
                .exists()
        );
    }

    fn fail(&mut self, message: &str) -> ! {
        let diagnostic = self.session.redacted_diagnostic();
        self.artifacts.push_transcript(&self.session.transcript());
        let metadata = format!("error={message}\nevents={:?}", self.session.events());
        let artifact = self.artifacts.persist(&diagnostic, &metadata);
        self.artifacts_persisted = true;
        match artifact {
            Ok(path) => panic!("{message}\n{diagnostic}\nartifacts={path:?}"),
            Err(error) => {
                panic!("{message}\n{diagnostic}\nfailed to persist sanitized artifacts: {error}")
            }
        }
    }
}

fn wait_for_process_group_exit(group: i32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let alive = Command::new("kill")
            .args(["-0", &format!("-{group}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !alive {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "PTY process group {group} remained after scenario exit"
        );
        thread::yield_now();
    }
}

impl Drop for RealTvScenario {
    fn drop(&mut self) {
        if !std::thread::panicking() || self.artifacts_persisted {
            return;
        }
        let diagnostic = self.session.redacted_diagnostic();
        self.artifacts.push_transcript(&self.session.transcript());
        self.artifacts.push_event(SemanticEvent::Note {
            message: "scenario dropped during panic".into(),
        });
        match self.artifacts.persist(&diagnostic, "panic=true") {
            Ok(Some(path)) => eprintln!("sanitized jtv failure artifacts: {}", path.display()),
            Ok(None) => eprintln!("jtv failure artifact preservation is disabled"),
            Err(error) => eprintln!("failed to persist sanitized jtv artifacts: {error}"),
        }
        self.artifacts_persisted = true;
    }
}

pub fn require_real_tools() -> io::Result<()> {
    let just = real_tool_path("just")?;
    check_version(&just, "just 1.53.0", &[])?;
    // Television needs a writable configuration directory even for --version.
    let home = tempfile::tempdir()?;
    let tv = real_tool_path("tv")?;
    check_version_with_home(&tv, "television 0.15.9", home.path())
}

pub fn real_tool_path(name: &str) -> io::Result<PathBuf> {
    match name {
        "just" => locate_program("JTV_TEST_REAL_JUST", name),
        "tv" => locate_program("JTV_TEST_REAL_TV", name),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown real test tool {name:?}"),
        )),
    }
}

fn check_version(program: &Path, expected: &str, extra: &[&str]) -> io::Result<()> {
    let output = Command::new(program)
        .args(extra)
        .arg("--version")
        .output()
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is required: {error}", program.display()),
            )
        })?;
    validate_version(&program.display().to_string(), expected, &output)
}

fn check_version_with_home(program: &Path, expected: &str, home: &Path) -> io::Result<()> {
    let output = Command::new(program)
        .arg("--version")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .output()
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is required: {error}", program.display()),
            )
        })?;
    validate_version(&program.display().to_string(), expected, &output)
}

fn locate_program(override_env: &str, name: &str) -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os(override_env) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{override_env} points to missing executable {}",
                path.display()
            ),
        ));
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{name} is required on PATH (or set {override_env})"),
            )
        })
}

fn validate_version(
    program: &str,
    expected: &str,
    output: &std::process::Output,
) -> io::Result<()> {
    let actual = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if output.status.success() && actual == expected {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!(
            "real-TUI tests require exactly {expected}; `{program} --version` returned {actual:?}. Install the pinned tool and rerun with `cargo test --test tui_workflows -- --ignored --test-threads=1`."
        ),
    ))
}

fn install_channel(sandbox: &TestSandbox, event_log: &Path) -> io::Result<()> {
    let mut command = configured_command(sandbox, event_log, env!("CARGO_BIN_EXE_jtv"));
    let output = command.arg("init").output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "jtv init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

fn configured_command(
    sandbox: &TestSandbox,
    event_log: &Path,
    program: impl AsRef<OsStr>,
) -> Command {
    let mut command = sandbox.command(program);
    for (key, value) in scenario_environment(sandbox, event_log) {
        command.env(key, value);
    }
    command
}

fn jtv_command(sandbox: &TestSandbox, event_log: &Path, columns: u16, rows: u16) -> PtyCommand {
    let mut command = PtyCommand::new(env!("CARGO_BIN_EXE_jtv"), sandbox.project())
        .arg("--justfile")
        .arg("justfile")
        .viewport(columns, rows);
    for (key, value) in scenario_environment(sandbox, event_log) {
        command = command.env(key, value);
    }
    command
}

fn scenario_environment(sandbox: &TestSandbox, event_log: &Path) -> BTreeMap<OsString, OsString> {
    let mut env = sandbox.environment();
    let just = sandbox_tool(
        sandbox,
        "just",
        &real_tool_path("just").expect("real just checked"),
    );
    let tv = sandbox_tool(
        sandbox,
        "tv",
        &real_tool_path("tv").expect("real TV checked"),
    );
    env.insert("JTV_JUST".into(), just.into_os_string());
    env.insert("JTV_TV".into(), tv.into_os_string());
    env.insert("JTV_E2E_LOG".into(), event_log.as_os_str().to_owned());
    env.insert("JTV_E2E_EXPR".into(), "expression-fallback".into());
    let jtv_dir = Path::new(env!("CARGO_BIN_EXE_jtv")).parent().unwrap();
    let host_path = std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into());
    let paths = std::iter::once(jtv_dir.to_path_buf()).chain(std::env::split_paths(&host_path));
    env.insert(
        "PATH".into(),
        std::env::join_paths(paths).unwrap_or(host_path),
    );
    env
}

fn sandbox_tool(sandbox: &TestSandbox, name: &str, source: &Path) -> PathBuf {
    let directory = sandbox.root().join("tools");
    fs::create_dir_all(&directory).expect("create stable sandbox tool directory");
    let destination = directory.join(name);
    if !destination.exists() {
        std::os::unix::fs::symlink(source, &destination).expect("symlink real tool into sandbox");
    }
    destination
}

fn copy_fixture(sandbox: &TestSandbox) -> io::Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/e2e");
    for name in ["justfile", ".jtv.toml", "sample.txt", "ops.just"] {
        sandbox.write_project_file(name, fs::read(source.join(name))?)?;
    }
    fs::create_dir_all(sandbox.project().join("sample-directory"))?;
    sandbox.write_project_file("sample-directory/marker.txt", b"directory fixture")?;
    Ok(())
}
