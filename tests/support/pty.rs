use std::collections::{BTreeMap, VecDeque};
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::PathBuf;
#[cfg(any(unix, windows))]
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, ExitStatus, MasterPty, PtySize};

use super::keys::Key;
use super::screen::ScreenFrame;

const MAX_TRANSCRIPT_BYTES: usize = 256 * 1024;
const MAX_EVENTS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyEvent {
    Output { bytes: usize },
    Text { value: String },
    SecretInput,
    Key { name: String },
    Resize { columns: u16, rows: u16 },
    Condition { description: String },
    Exit { code: u32, signal: Option<String> },
    Timeout { operation: String },
}

#[derive(Debug, Clone)]
pub struct PtyCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: BTreeMap<OsString, OsString>,
    pub viewport: PtySize,
}

impl PtyCommand {
    pub fn new(program: impl AsRef<OsStr>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            program: program.as_ref().to_owned(),
            args: Vec::new(),
            cwd: cwd.into(),
            env: BTreeMap::new(),
            viewport: PtySize {
                rows: 40,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            },
        }
    }

    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .insert(key.as_ref().to_owned(), value.as_ref().to_owned());
        self
    }

    pub fn viewport(mut self, columns: u16, rows: u16) -> Self {
        self.viewport.cols = columns;
        self.viewport.rows = rows;
        self
    }
}

struct Shared {
    parser: vt100::Parser,
    transcript: VecDeque<u8>,
    events: VecDeque<PtyEvent>,
    secrets: Vec<String>,
    generation: u64,
    reader_done: bool,
    reader_error: Option<String>,
}

struct State {
    shared: Mutex<Shared>,
    changed: Condvar,
}

pub struct PtySession {
    child: Option<Box<dyn Child + Send + Sync>>,
    #[cfg(unix)]
    process_group: Option<i32>,
    master: Box<dyn MasterPty + Send>,
    writer: Option<Box<dyn Write + Send>>,
    state: Arc<State>,
    reader: Option<JoinHandle<()>>,
    metadata: PtyCommand,
    status: Option<ExitStatus>,
}

impl PtySession {
    pub fn spawn(command: PtyCommand) -> io::Result<Self> {
        if command.viewport.rows == 0 || command.viewport.cols == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "PTY viewport dimensions must be nonzero",
            ));
        }
        let pair = portable_pty::native_pty_system()
            .openpty(command.viewport)
            .map_err(other)?;
        let mut builder = CommandBuilder::new(&command.program);
        builder.args(&command.args);
        builder.cwd(&command.cwd);
        builder.env_clear();
        for (key, value) in &command.env {
            builder.env(key, value);
        }
        let child = pair.slave.spawn_command(builder).map_err(other)?;
        #[cfg(unix)]
        let process_group = child.process_id().map(|pid| pid as i32);
        // Keeping a slave descriptor open prevents EOF on some PTY implementations.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(other)?;
        let writer = pair.master.take_writer().map_err(other)?;
        let state = Arc::new(State {
            shared: Mutex::new(Shared {
                parser: vt100::Parser::new(command.viewport.rows, command.viewport.cols, 0),
                transcript: VecDeque::new(),
                events: VecDeque::new(),
                secrets: Vec::new(),
                generation: 0,
                reader_done: false,
                reader_error: None,
            }),
            changed: Condvar::new(),
        });
        let reader_state = Arc::clone(&state);
        let reader_thread = thread::Builder::new()
            .name("jtv-test-pty-reader".into())
            .spawn(move || reader_loop(&mut *reader, &reader_state))?;
        Ok(Self {
            child: Some(child),
            #[cfg(unix)]
            process_group,
            master: pair.master,
            writer: Some(writer),
            state,
            reader: Some(reader_thread),
            metadata: command,
            status: None,
        })
    }

    pub fn send_text(&mut self, text: &str) -> io::Result<()> {
        self.write_input(text.as_bytes())?;
        self.push_event(PtyEvent::Text {
            value: text.to_owned(),
        });
        Ok(())
    }

    pub fn send_secret(&mut self, secret: &str) -> io::Result<()> {
        {
            let mut shared = self.state.shared.lock().unwrap_or_else(|e| e.into_inner());
            if !secret.is_empty() && !shared.secrets.iter().any(|value| value == secret) {
                shared.secrets.push(secret.to_owned());
            }
        }
        self.write_input(secret.as_bytes())?;
        self.push_event(PtyEvent::SecretInput);
        Ok(())
    }

    pub fn send_key(&mut self, key: Key) -> io::Result<()> {
        self.write_input(&key.bytes())?;
        self.push_event(PtyEvent::Key { name: key.name() });
        Ok(())
    }

    pub fn interrupt(&mut self) -> io::Result<()> {
        self.send_key(Key::Ctrl('c'))
    }

    pub fn resize(&mut self, columns: u16, rows: u16) -> io::Result<()> {
        if columns == 0 || rows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "PTY viewport dimensions must be nonzero",
            ));
        }
        self.master
            .resize(PtySize {
                rows,
                cols: columns,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(other)?;
        let mut shared = self.state.shared.lock().unwrap_or_else(|e| e.into_inner());
        shared.parser.screen_mut().set_size(rows, columns);
        push_event(&mut shared.events, PtyEvent::Resize { columns, rows });
        shared.generation = shared.generation.wrapping_add(1);
        self.state.changed.notify_all();
        Ok(())
    }

    pub fn frame(&self) -> ScreenFrame {
        let shared = self.state.shared.lock().unwrap_or_else(|e| e.into_inner());
        ScreenFrame::from_parser(&shared.parser)
    }

    pub fn transcript(&self) -> Vec<u8> {
        self.state
            .shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .transcript
            .iter()
            .copied()
            .collect()
    }

    pub fn events(&self) -> Vec<PtyEvent> {
        self.state
            .shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .events
            .iter()
            .cloned()
            .collect()
    }

    #[allow(dead_code)]
    pub fn redacted_diagnostic(&self) -> String {
        let shared = self.state.shared.lock().unwrap_or_else(|e| e.into_inner());
        redact(
            ScreenFrame::from_parser(&shared.parser).diagnostic(),
            &shared.secrets,
        )
    }

    #[cfg(unix)]
    pub fn process_group_leader(&self) -> Option<i32> {
        self.process_group
    }

    pub fn wait_for_screen<F>(
        &self,
        description: impl Into<String>,
        timeout: Duration,
        predicate: F,
    ) -> io::Result<ScreenFrame>
    where
        F: Fn(&ScreenFrame) -> bool,
    {
        let description = description.into();
        let deadline = Instant::now() + timeout;
        let mut shared = self.state.shared.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            let frame = ScreenFrame::from_parser(&shared.parser);
            if predicate(&frame) {
                push_event(
                    &mut shared.events,
                    PtyEvent::Condition {
                        description: description.clone(),
                    },
                );
                return Ok(frame);
            }
            if shared.reader_done {
                return Err(self.diagnostic_error(
                    io::ErrorKind::UnexpectedEof,
                    &format!("reader ended before screen condition {description:?}"),
                    &shared,
                ));
            }
            let now = Instant::now();
            if now >= deadline {
                push_event(
                    &mut shared.events,
                    PtyEvent::Timeout {
                        operation: description.clone(),
                    },
                );
                return Err(self.diagnostic_error(
                    io::ErrorKind::TimedOut,
                    &format!("timed out waiting for screen condition {description:?}"),
                    &shared,
                ));
            }
            let generation = shared.generation;
            let (next, _) = self
                .state
                .changed
                .wait_timeout(shared, deadline.saturating_duration_since(now))
                .unwrap_or_else(|e| e.into_inner());
            shared = next;
            if shared.generation == generation && Instant::now() >= deadline {
                continue;
            }
        }
    }

    /// Waits until no terminal output has arrived for `quiet_period`.
    ///
    /// This is intended only after a semantic screen condition has succeeded;
    /// it makes snapshot capture deterministic without treating an elapsed
    /// sleep as proof that the expected UI appeared.
    pub fn wait_for_quiet(
        &self,
        quiet_period: Duration,
        timeout: Duration,
    ) -> io::Result<ScreenFrame> {
        let deadline = Instant::now() + timeout;
        let mut quiet_deadline = Instant::now() + quiet_period;
        let mut shared = self.state.shared.lock().unwrap_or_else(|e| e.into_inner());
        let mut generation = shared.generation;
        loop {
            let now = Instant::now();
            if now >= quiet_deadline {
                push_event(
                    &mut shared.events,
                    PtyEvent::Condition {
                        description: format!("screen quiet for {quiet_period:?}"),
                    },
                );
                return Ok(ScreenFrame::from_parser(&shared.parser));
            }
            if now >= deadline {
                push_event(
                    &mut shared.events,
                    PtyEvent::Timeout {
                        operation: "screen settle".into(),
                    },
                );
                return Err(self.diagnostic_error(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for the screen to settle",
                    &shared,
                ));
            }
            let wait = quiet_deadline
                .saturating_duration_since(now)
                .min(deadline.saturating_duration_since(now));
            let (next, _) = self
                .state
                .changed
                .wait_timeout(shared, wait)
                .unwrap_or_else(|e| e.into_inner());
            shared = next;
            if shared.generation != generation {
                generation = shared.generation;
                quiet_deadline = Instant::now() + quiet_period;
            }
        }
    }

    pub fn wait_for_exit(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        if let Some(status) = &self.status {
            return Ok(status.clone());
        }
        let deadline = Instant::now() + timeout;
        loop {
            let child = self.child.as_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::BrokenPipe, "PTY child is unavailable")
            })?;
            if let Some(status) = child.try_wait()? {
                self.record_exit(&status);
                self.status = Some(status.clone());
                return Ok(status);
            }
            let now = Instant::now();
            if now >= deadline {
                self.push_event(PtyEvent::Timeout {
                    operation: "child exit".into(),
                });
                return Err(self.timeout_error("timed out waiting for PTY child to exit"));
            }
            let shared = self.state.shared.lock().unwrap_or_else(|e| e.into_inner());
            let _ = self
                .state
                .changed
                .wait_timeout(
                    shared,
                    deadline
                        .saturating_duration_since(now)
                        .min(Duration::from_millis(25)),
                )
                .unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn terminate(&mut self) -> io::Result<ExitStatus> {
        let recorded_status = self.status.clone();
        #[cfg(unix)]
        if let Some(group) = self
            .process_group
            .filter(|group| process_group_alive(*group))
        {
            signal_process_group(group, "-TERM");
            let deadline = Instant::now() + Duration::from_millis(250);
            let mut direct_status = recorded_status.clone();
            while process_group_alive(group) && Instant::now() < deadline {
                if direct_status.is_none() {
                    direct_status = self
                        .child
                        .as_mut()
                        .ok_or_else(|| {
                            io::Error::new(io::ErrorKind::BrokenPipe, "PTY child is unavailable")
                        })?
                        .try_wait()?;
                }
                thread::yield_now();
            }
            if process_group_alive(group) {
                signal_process_group(group, "-KILL");
            }
            if let Some(status) = direct_status {
                self.record_exit(&status);
                self.status = Some(status.clone());
                return Ok(status);
            }
        }
        #[cfg(windows)]
        if let Some(pid) = self.child.as_ref().and_then(|child| child.process_id()) {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        if let Some(status) = recorded_status {
            return Ok(status);
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let status = child.wait()?;
            self.record_exit(&status);
            self.status = Some(status.clone());
            return Ok(status);
        }
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "PTY child is unavailable",
        ))
    }

    fn write_input(&mut self, bytes: &[u8]) -> io::Result<()> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "PTY input is closed"))?;
        writer.write_all(bytes)?;
        writer.flush()
    }

    fn push_event(&self, event: PtyEvent) {
        let mut shared = self.state.shared.lock().unwrap_or_else(|e| e.into_inner());
        push_event(&mut shared.events, event);
    }

    fn record_exit(&self, status: &ExitStatus) {
        self.push_event(PtyEvent::Exit {
            code: status.exit_code(),
            signal: status.signal().map(str::to_owned),
        });
    }

    fn timeout_error(&self, message: &str) -> io::Error {
        let shared = self.state.shared.lock().unwrap_or_else(|e| e.into_inner());
        self.diagnostic_error(io::ErrorKind::TimedOut, message, &shared)
    }

    fn diagnostic_error(&self, kind: io::ErrorKind, message: &str, shared: &Shared) -> io::Error {
        let frame = ScreenFrame::from_parser(&shared.parser);
        let transcript = redact(
            String::from_utf8_lossy(&shared.transcript.iter().copied().collect::<Vec<_>>())
                .into_owned(),
            &shared.secrets,
        );
        let screen = redact(frame.diagnostic(), &shared.secrets);
        io::Error::new(
            kind,
            format!(
                "{message}\nprogram={:?} args={:?} cwd={} viewport={}x{} reader_error={:?}\n{}\nrecent events={:?}\ntranscript tail={transcript:?}",
                self.metadata.program,
                self.metadata.args,
                self.metadata.cwd.display(),
                frame.columns,
                frame.rows,
                shared.reader_error,
                screen,
                shared.events,
            ),
        )
    }
}

fn redact(mut value: String, secrets: &[String]) -> String {
    for secret in secrets {
        value = value.replace(secret, "[REDACTED]");
    }
    value
}

impl Drop for PtySession {
    fn drop(&mut self) {
        self.writer.take();
        let _ = self.terminate();
        self.child.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(unix)]
fn signal_process_group(group: i32, signal: &str) {
    let _ = Command::new("kill")
        .args([signal, &format!("-{group}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(unix)]
fn process_group_alive(group: i32) -> bool {
    Command::new("kill")
        .args(["-0", &format!("-{group}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn reader_loop(reader: &mut dyn Read, state: &State) {
    let mut buffer = [0_u8; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(length) => {
                let mut shared = state.shared.lock().unwrap_or_else(|e| e.into_inner());
                shared.parser.process(&buffer[..length]);
                push_bounded(&mut shared.transcript, &buffer[..length]);
                push_event(&mut shared.events, PtyEvent::Output { bytes: length });
                shared.generation = shared.generation.wrapping_add(1);
                state.changed.notify_all();
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let mut shared = state.shared.lock().unwrap_or_else(|e| e.into_inner());
                shared.reader_error = Some(error.to_string());
                break;
            }
        }
    }
    let mut shared = state.shared.lock().unwrap_or_else(|e| e.into_inner());
    shared.reader_done = true;
    shared.generation = shared.generation.wrapping_add(1);
    state.changed.notify_all();
}

fn push_bounded(transcript: &mut VecDeque<u8>, bytes: &[u8]) {
    if bytes.len() >= MAX_TRANSCRIPT_BYTES {
        transcript.clear();
        transcript.extend(&bytes[bytes.len() - MAX_TRANSCRIPT_BYTES..]);
        return;
    }
    let overflow = transcript
        .len()
        .saturating_add(bytes.len())
        .saturating_sub(MAX_TRANSCRIPT_BYTES);
    transcript.drain(..overflow);
    transcript.extend(bytes);
}

fn push_event(events: &mut VecDeque<PtyEvent>, event: PtyEvent) {
    if events.len() == MAX_EVENTS {
        events.pop_front();
    }
    events.push_back(event);
}

fn other(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}
