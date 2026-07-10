use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_TEXT_BYTES: usize = 256 * 1024;
const MAX_EVENTS: usize = 512;

#[derive(Debug, Clone)]
pub struct Sanitizer {
    sandbox_root: PathBuf,
    workspace_root: Option<PathBuf>,
    secrets: Vec<String>,
}

impl Sanitizer {
    pub fn new(sandbox_root: impl Into<PathBuf>) -> Self {
        Self {
            sandbox_root: sandbox_root.into(),
            workspace_root: None,
            secrets: Vec::new(),
        }
    }
    pub fn add_workspace_root(&mut self, workspace_root: impl Into<PathBuf>) {
        self.workspace_root = Some(workspace_root.into());
    }
    pub fn add_secret(&mut self, secret: impl Into<String>) {
        let secret = secret.into();
        if !secret.is_empty() && !self.secrets.contains(&secret) {
            self.secrets.push(secret);
            self.secrets
                .sort_by_key(|value| std::cmp::Reverse(value.len()));
        }
    }
    pub fn sanitize(&self, value: &str) -> String {
        let mut sanitized = value.replace('\0', "�");
        for secret in &self.secrets {
            sanitized = sanitized.replace(secret, "<SECRET>");
        }
        let native_root = self.sandbox_root.to_string_lossy();
        sanitized = sanitized.replace(native_root.as_ref(), "<SANDBOX>");
        let slash_root = native_root.replace('\\', "/");
        sanitized = sanitized.replace(&slash_root, "<SANDBOX>");
        if let Some(workspace_root) = &self.workspace_root {
            let native_root = workspace_root.to_string_lossy();
            sanitized = sanitized.replace(native_root.as_ref(), "<WORKSPACE>");
            sanitized = sanitized.replace(&native_root.replace('\\', "/"), "<WORKSPACE>");
        }
        sanitized.replace('\\', "/")
    }
    pub fn contains_secret(&self, value: &[u8]) -> bool {
        self.secrets.iter().any(|secret| {
            !secret.is_empty()
                && value
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticEvent {
    Spawn { program: String, args: Vec<String> },
    Key { name: String },
    Text { value: String },
    SecretInput,
    Resize { columns: u16, rows: u16 },
    ScreenCondition { description: String },
    Exit { status: Option<i32> },
    Note { message: String },
}

impl SemanticEvent {
    fn render(&self) -> String {
        match self {
            Self::Spawn { program, args } => format!("spawn\t{program}\t{}", args.join("\t")),
            Self::Key { name } => format!("key\t{name}"),
            Self::Text { value } => format!("text\t{value}"),
            Self::SecretInput => "secret-input\t<REDACTED>".into(),
            Self::Resize { columns, rows } => format!("resize\t{columns}x{rows}"),
            Self::ScreenCondition { description } => format!("screen-condition\t{description}"),
            Self::Exit { status } => format!(
                "exit\t{}",
                status.map_or_else(|| "signal".into(), |value| value.to_string())
            ),
            Self::Note { message } => format!("note\t{message}"),
        }
    }
}

#[derive(Debug)]
pub struct FailureArtifacts {
    scenario: String,
    output_root: PathBuf,
    preserve: bool,
    sanitizer: Sanitizer,
    transcript: VecDeque<u8>,
    events: VecDeque<SemanticEvent>,
}

impl FailureArtifacts {
    pub fn new(
        scenario: &str,
        workspace: &Path,
        sandbox_root: &Path,
        preserve: bool,
    ) -> io::Result<Self> {
        let mut sanitizer = Sanitizer::new(sandbox_root);
        sanitizer.add_workspace_root(workspace);
        Ok(Self {
            scenario: safe_scenario_name(scenario)?,
            output_root: workspace.join("target").join("jtv-test-artifacts"),
            preserve,
            sanitizer,
            transcript: VecDeque::new(),
            events: VecDeque::new(),
        })
    }
    pub fn sanitizer_mut(&mut self) -> &mut Sanitizer {
        &mut self.sanitizer
    }
    pub fn push_transcript(&mut self, bytes: &[u8]) {
        push_bounded(&mut self.transcript, bytes, MAX_TEXT_BYTES);
    }
    pub fn push_event(&mut self, event: SemanticEvent) {
        if self.events.len() == MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
    pub fn persist(&mut self, final_screen: &str, metadata: &str) -> io::Result<Option<PathBuf>> {
        if !self.preserve {
            return Ok(None);
        }
        let directory = self.output_root.join(&self.scenario);
        fs::create_dir_all(&directory)?;
        let transcript = String::from_utf8_lossy(self.transcript.make_contiguous());
        let events = self
            .events
            .iter()
            .map(SemanticEvent::render)
            .collect::<Vec<_>>()
            .join("\n");
        let files = [
            ("transcript.txt", self.sanitizer.sanitize(&transcript)),
            ("screen.txt", self.sanitizer.sanitize(final_screen)),
            ("events.tsv", self.sanitizer.sanitize(&events)),
            ("metadata.txt", self.sanitizer.sanitize(metadata)),
        ];
        for (name, contents) in files {
            if self.sanitizer.contains_secret(contents.as_bytes()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("secret sentinel remained in {name}"),
                ));
            }
            fs::write(
                directory.join(name),
                bounded_string(&contents, MAX_TEXT_BYTES),
            )?;
        }
        Ok(Some(directory))
    }
}

fn push_bounded(buffer: &mut VecDeque<u8>, bytes: &[u8], maximum: usize) {
    if bytes.len() >= maximum {
        buffer.clear();
        buffer.extend(&bytes[bytes.len() - maximum..]);
        return;
    }
    let overflow = buffer
        .len()
        .saturating_add(bytes.len())
        .saturating_sub(maximum);
    buffer.drain(..overflow);
    buffer.extend(bytes);
}
fn bounded_string(value: &str, maximum: usize) -> &str {
    if value.len() <= maximum {
        return value;
    }
    let mut start = value.len() - maximum;
    while !value.is_char_boundary(start) {
        start += 1;
    }
    &value[start..]
}
fn safe_scenario_name(name: &str) -> io::Result<String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scenario name must contain only ASCII letters, digits, '-' or '_'",
        ));
    }
    Ok(name.to_owned())
}
