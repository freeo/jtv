//! Optional integrations with shell-owned history stores.
//!
//! This module deliberately does not discover or edit Atuin's database.  An
//! opt-in shell wrapper resolves the active Atuin executable and passes it to
//! jtv.  The runner can then use Atuin's public start/end lifecycle around an
//! eligible command attempt.

use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

use crate::{
    command::CommandPlan,
    runner::{AttemptOutcome, ExecutionObserver},
};

const INTEGRATION_ENV: &str = "JTV_SHELL_INTEGRATION";
const PROTOCOL_ENV: &str = "JTV_HISTORY_PROTOCOL";
const SESSION_ENV: &str = "JTV_HISTORY_SESSION";
const ATUIN_BIN_ENV: &str = "JTV_ATUIN_BIN";
const ATUIN_SESSION_ENV: &str = "ATUIN_SESSION";
#[cfg(unix)]
const HISTORY_SINK_ENV: &str = "JTV_HISTORY_SINK";
const ZSH_INTEGRATION: &str = "zsh";
const HISTORY_PROTOCOL: &str = "1";
const MAX_HISTORY_ID_BYTES: usize = 128;
const MAX_HISTORY_BYTES: u64 = 1024 * 1024;
const MAX_HISTORY_ENTRIES: usize = 1024;
const ATUIN_TIMEOUT: Duration = Duration::from_secs(2);

fn integration_environment_active() -> bool {
    std::env::var_os(INTEGRATION_ENV).as_deref() == Some(OsStr::new(ZSH_INTEGRATION))
        && std::env::var_os(PROTOCOL_ENV).as_deref() == Some(OsStr::new(HISTORY_PROTOCOL))
        && std::env::var_os(SESSION_ENV)
            .as_deref()
            .is_some_and(valid_session)
}

fn valid_session(value: &OsStr) -> bool {
    let bytes = value.as_encoded_bytes();
    bytes.starts_with(b"jtv-history.")
        && bytes.len() <= 128
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

/// Best-effort observer activated only by the generated zsh wrapper.
pub struct HistoryObserver {
    sink: Option<File>,
    atuin: Option<AtuinClient>,
    pending: Option<PendingEntry>,
    entries: usize,
}

struct PendingEntry {
    command: String,
    atuin: Option<AtuinEntry>,
}

impl HistoryObserver {
    #[must_use]
    pub fn from_environment() -> Self {
        let active = integration_environment_active();
        Self {
            sink: active.then(open_history_sink).flatten(),
            atuin: AtuinClient::from_environment(),
            pending: None,
            entries: 0,
        }
    }

    fn append(&mut self, command: &str) {
        if self.entries >= MAX_HISTORY_ENTRIES
            || command
                .chars()
                .any(|character| matches!(character, '\n' | '\r' | '\0'))
        {
            return;
        }
        let Some(sink) = &mut self.sink else {
            return;
        };
        let Ok(metadata) = sink.metadata() else {
            self.sink = None;
            return;
        };
        let additional = command.len().saturating_add(1) as u64;
        if metadata.len().saturating_add(additional) > MAX_HISTORY_BYTES
            || sink.write_all(command.as_bytes()).is_err()
            || sink.write_all(b"\n").is_err()
            || sink.flush().is_err()
        {
            self.sink = None;
            return;
        }
        self.entries += 1;
    }
}

impl ExecutionObserver for HistoryObserver {
    fn before(&mut self, plan: &CommandPlan) {
        self.pending = plan
            .history_command_zsh()
            .ok()
            .flatten()
            .map(|command| PendingEntry {
                atuin: self
                    .atuin
                    .as_ref()
                    .and_then(|client| client.start(&command, &plan.cwd)),
                command,
            });
    }

    fn after(&mut self, _plan: &CommandPlan, outcome: AttemptOutcome<'_>, duration: Duration) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let status = match outcome {
            AttemptOutcome::Exit(status) => status,
            AttemptOutcome::Error(_) => 1,
        };
        if let Some(entry) = pending.atuin {
            entry.finish(status, duration);
        }
        self.append(&pending.command);
    }
}

#[cfg(unix)]
fn open_history_sink() -> Option<File> {
    let path = PathBuf::from(std::env::var_os(HISTORY_SINK_ENV)?);
    let session = std::env::var_os(SESSION_ENV)?;
    open_history_sink_path(&path, &session)
}

#[cfg(unix)]
fn open_history_sink_path(path: &Path, session: &OsStr) -> Option<File> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    if !valid_session(session) || path.file_name() != Some(session) {
        return None;
    }
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
        || metadata.len() > MAX_HISTORY_BYTES
    {
        return None;
    }
    let file = OpenOptions::new()
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .ok()?;
    let opened = file.metadata().ok()?;
    if !opened.is_file()
        || opened.uid() != unsafe { libc::geteuid() }
        || opened.mode() & 0o077 != 0
        || opened.dev() != metadata.dev()
        || opened.ino() != metadata.ino()
    {
        return None;
    }
    Some(file)
}

#[cfg(not(unix))]
fn open_history_sink() -> Option<File> {
    None
}

/// Best-effort adapter for Atuin's supported `history start`/`history end` API.
#[derive(Clone, Debug)]
pub struct AtuinClient {
    executable: PathBuf,
}

/// A history entry begun by [`AtuinClient::start`].
#[derive(Debug)]
pub struct AtuinEntry {
    client: AtuinClient,
    id: String,
    cwd: PathBuf,
}

impl AtuinClient {
    /// Detect Atuin only when the opt-in zsh wrapper explicitly activated it.
    ///
    /// `$SHELL` and ambient `PATH` are intentionally ignored: standalone jtv
    /// must remain inert, and the wrapper knows which shell and Atuin instance
    /// are actually active.
    #[must_use]
    pub fn from_environment() -> Option<Self> {
        if !integration_environment_active() {
            return None;
        }
        Self::from_values(
            std::env::var_os(INTEGRATION_ENV).as_deref(),
            std::env::var_os(ATUIN_BIN_ENV),
            std::env::var_os(ATUIN_SESSION_ENV).as_deref(),
        )
    }

    fn from_values(
        integration: Option<&OsStr>,
        executable: Option<OsString>,
        session: Option<&OsStr>,
    ) -> Option<Self> {
        if integration != Some(OsStr::new(ZSH_INTEGRATION)) || session.is_none_or(OsStr::is_empty) {
            return None;
        }
        let executable = executable.filter(|value| !value.is_empty())?;
        Some(Self {
            executable: PathBuf::from(executable),
        })
    }

    /// Begin a synthetic Atuin record in the command's actual working directory.
    ///
    /// The executable command is passed as one opaque argument after `--`.
    /// Empty, malformed, or failed responses are treated as an unavailable
    /// integration and produce no output.
    #[must_use]
    pub fn start(&self, command_line: &str, cwd: &Path) -> Option<AtuinEntry> {
        let mut command = Command::new(&self.executable);
        command
            .args(["history", "start", "--"])
            .arg(command_line)
            .current_dir(cwd)
            // Atuin's shell-oriented CLI reads PWD as its directory metadata;
            // `Command::current_dir` alone does not update that environment
            // variable inherited from jtv's parent shell.
            .env("PWD", cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let output = bounded_output(&mut command)?;
        if !output.status.success() {
            return None;
        }

        let id = parse_history_id(&output.stdout)?;
        Some(AtuinEntry {
            client: self.clone(),
            id,
            cwd: cwd.to_path_buf(),
        })
    }
}

impl AtuinEntry {
    /// Finish this record with the attempted command's real result.
    ///
    /// Errors are intentionally discarded: history is auxiliary and must not
    /// alter jtv's status or write into its terminal streams.
    pub fn finish(self, status: i32, duration: Duration) {
        let nanos = duration.as_nanos();
        let mut command = Command::new(&self.client.executable);
        command
            .args(["history", "end", "--exit"])
            .arg(status.to_string())
            .arg("--duration")
            .arg(nanos.to_string())
            .arg("--")
            .arg(self.id)
            .current_dir(&self.cwd)
            .env("PWD", &self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _ = bounded_output(&mut command);
    }
}

fn bounded_output(command: &mut Command) -> Option<Output> {
    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + ATUIN_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait_with_output();
                return None;
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn parse_history_id(stdout: &[u8]) -> Option<String> {
    if stdout.len() > MAX_HISTORY_ID_BYTES {
        return None;
    }
    let id = std::str::from_utf8(stdout).ok()?.trim();
    if id.is_empty()
        || id.len() > MAX_HISTORY_ID_BYTES
        || !id.as_bytes()[0].is_ascii_alphanumeric()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return None;
    }
    Some(id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::runner::ExecutionObserver;

    #[test]
    fn detection_requires_explicit_zsh_integration_active_session_and_binary() {
        let bin = Some(OsString::from("/usr/bin/atuin"));
        assert!(
            AtuinClient::from_values(
                Some(OsStr::new("zsh")),
                bin.clone(),
                Some(OsStr::new("session"))
            )
            .is_some()
        );
        assert!(AtuinClient::from_values(None, bin.clone(), Some(OsStr::new("session"))).is_none());
        assert!(
            AtuinClient::from_values(
                Some(OsStr::new("bash")),
                bin.clone(),
                Some(OsStr::new("session"))
            )
            .is_none()
        );
        assert!(
            AtuinClient::from_values(Some(OsStr::new("zsh")), None, Some(OsStr::new("session")))
                .is_none()
        );
        assert!(AtuinClient::from_values(Some(OsStr::new("zsh")), bin.clone(), None).is_none());
        assert!(
            AtuinClient::from_values(Some(OsStr::new("zsh")), bin, Some(OsStr::new(""))).is_none()
        );
    }

    #[test]
    fn history_ids_are_bounded_and_never_interpreted_as_options() {
        assert_eq!(
            parse_history_id(b"019f523af3dc7ea088d592c62b0b0083\n").as_deref(),
            Some("019f523af3dc7ea088d592c62b0b0083")
        );
        assert!(parse_history_id(b"").is_none());
        assert!(parse_history_id(b"--delete-everything").is_none());
        assert!(parse_history_id(b"id with spaces").is_none());
        assert!(parse_history_id(&[b'a'; MAX_HISTORY_ID_BYTES + 1]).is_none());
    }

    #[test]
    fn io_error_is_a_best_effort_noop() {
        let client = AtuinClient {
            executable: PathBuf::from("/definitely/missing/jtv-atuin"),
        };
        assert!(client.start("just test", Path::new("/")).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn observer_appends_only_eligible_attempts_to_a_private_sink() {
        use std::os::unix::fs::PermissionsExt;

        let sandbox = tempfile::tempdir().unwrap();
        let path = sandbox.path().join("jtv-history.observer-test");
        std::fs::write(&path, []).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut observer = HistoryObserver {
            sink: open_history_sink_path(&path, path.file_name().unwrap()),
            atuin: None,
            pending: None,
            entries: 0,
        };
        let eligible = CommandPlan {
            program: "just".into(),
            cwd: sandbox.path().into(),
            args: vec!["deploy".into(), "production".into()],
            redacted_args: vec![],
            contains_secret: false,
        };
        observer.before(&eligible);
        observer.after(
            &eligible,
            AttemptOutcome::Exit(17),
            Duration::from_millis(4),
        );
        let secret = CommandPlan {
            contains_secret: true,
            ..eligible.clone()
        };
        observer.before(&secret);
        observer.after(&secret, AttemptOutcome::Exit(0), Duration::from_millis(1));
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "just deploy production\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn sink_rejects_permissive_files_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let sandbox = tempfile::tempdir().unwrap();
        let path = sandbox.path().join("jtv-history.sink-test");
        std::fs::write(&path, []).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(open_history_sink_path(&path, path.file_name().unwrap()).is_none());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(open_history_sink_path(&path, path.file_name().unwrap()).is_some());
        let link = sandbox.path().join("jtv-history.link-test");
        symlink(&path, &link).unwrap();
        assert!(open_history_sink_path(&link, link.file_name().unwrap()).is_none());
        assert!(open_history_sink_path(&path, OsStr::new("jtv-history.other")).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn fake_atuin_receives_exact_start_and_end_contract() {
        use std::{fs, os::unix::fs::PermissionsExt};

        let sandbox = tempfile::tempdir().unwrap();
        let bin = sandbox.path().join("atuin");
        let log = sandbox.path().join("calls");
        let cwd = sandbox.path().join("project with spaces");
        fs::create_dir(&cwd).unwrap();
        let script = format!(
            r#"#!/bin/sh
printf 'cwd=%s\n' "$PWD" >> '{}'
for arg in "$@"; do printf 'arg=%s\n' "$arg" >> '{}'; done
if [ "$2" = start ]; then printf '%s\n' 019f523af3dc7ea088d592c62b0b0083; fi
"#,
            log.display(),
            log.display()
        );
        fs::write(&bin, script).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o700)).unwrap();

        let client = AtuinClient { executable: bin };
        let line = "just --justfile '/tmp/a b/justfile' deploy 'semi;colon'";
        let entry = client.start(line, &cwd).expect("valid fake Atuin ID");
        entry.finish(17, Duration::from_nanos(42));
        let calls = fs::read_to_string(log).unwrap();
        let expected = format!(
            "cwd={}\narg=history\narg=start\narg=--\narg={line}\n\
             cwd={}\narg=history\narg=end\narg=--exit\narg=17\n\
             arg=--duration\narg=42\narg=--\narg=019f523af3dc7ea088d592c62b0b0083\n",
            cwd.display(),
            cwd.display()
        );
        assert_eq!(calls, expected);
    }

    #[cfg(unix)]
    #[test]
    fn hung_atuin_is_bounded_and_silent() {
        use std::{fs, os::unix::fs::PermissionsExt};

        let sandbox = tempfile::tempdir().unwrap();
        let bin = sandbox.path().join("atuin");
        fs::write(&bin, "#!/bin/sh\nexec sleep 10\n").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o700)).unwrap();
        let client = AtuinClient { executable: bin };
        let started = Instant::now();
        assert!(client.start("just test", sandbox.path()).is_none());
        assert!(started.elapsed() < Duration::from_secs(4));
    }
}
