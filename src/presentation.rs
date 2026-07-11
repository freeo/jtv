//! Trusted terminal presentation primitives.
//!
//! Untrusted project text must enter through [`sanitize_inline`] or
//! [`sanitize_multiline`]. ANSI is emitted only by [`StyledText::ansi`], from
//! the fixed palette below, or accepted from an optional renderer through
//! [`validate_sgr_only`].

use std::env;
use std::fmt;

use serde::{Deserialize, Serialize};

const ESC: char = '\u{1b}';
const REPLACEMENT: char = '\u{fffd}';
const RESET: &str = "\x1b[0m";

/// Semantic styles corresponding to the archived jtv ANSI-16 palette.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StyleRole {
    Plain,
    Recipe,
    ParameterName,
    Required,
    DefaultValue,
    Dependency,
    ModuleHeader,
    ModuleDocker,
    ModuleTest,
    ModuleDeploy,
    ModuleDefault,
    PreviewTitle,
    Documentation,
    Attribute,
    Signature,
    Dim,
    Bold,
    Success,
    Warning,
    Error,
}

impl StyleRole {
    /// The exact SGR prefix for this role. Plain text has no prefix.
    pub const fn sgr(self) -> &'static str {
        match self {
            Self::Plain => "",
            Self::Recipe | Self::Signature => "\x1b[0;36m",
            Self::ParameterName => "\x1b[1;33m",
            Self::Required => "\x1b[0;91m",
            Self::DefaultValue | Self::Success => "\x1b[0;32m",
            Self::Dependency => "\x1b[0;95m",
            Self::ModuleHeader => "\x1b[1;37m",
            Self::ModuleDocker | Self::Attribute => "\x1b[0;94m",
            Self::ModuleTest => "\x1b[0;92m",
            Self::ModuleDeploy | Self::Warning => "\x1b[0;93m",
            Self::ModuleDefault => "\x1b[0;96m",
            Self::PreviewTitle => "\x1b[1;32m",
            Self::Documentation => "\x1b[0;35m",
            Self::Dim => "\x1b[2m",
            Self::Bold => "\x1b[1m",
            Self::Error => "\x1b[0;31m",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyledSpan {
    text: String,
    role: StyleRole,
}

impl StyledSpan {
    pub fn inline(text: impl AsRef<str>, role: StyleRole) -> Self {
        Self {
            text: sanitize_inline(text.as_ref()),
            role,
        }
    }

    pub fn multiline(text: impl AsRef<str>, role: StyleRole) -> Self {
        Self {
            text: sanitize_multiline(text.as_ref()),
            role,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub const fn role(&self) -> StyleRole {
        self.role
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StyledText {
    spans: Vec<StyledSpan>,
}

impl StyledText {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_span(span: StyledSpan) -> Self {
        Self { spans: vec![span] }
    }

    pub fn push(&mut self, span: StyledSpan) -> &mut Self {
        self.spans.push(span);
        self
    }

    pub fn inline(&mut self, text: impl AsRef<str>, role: StyleRole) -> &mut Self {
        self.push(StyledSpan::inline(text, role))
    }

    pub fn multiline(&mut self, text: impl AsRef<str>, role: StyleRole) -> &mut Self {
        self.push(StyledSpan::multiline(text, role))
    }

    /// Append one renderer-owned structural line break.
    pub fn newline(&mut self) -> &mut Self {
        self.spans.push(StyledSpan {
            text: "\n".to_owned(),
            role: StyleRole::Plain,
        });
        self
    }

    pub fn spans(&self) -> &[StyledSpan] {
        &self.spans
    }

    pub fn plain(&self) -> String {
        self.spans.iter().map(StyledSpan::text).collect()
    }

    /// Serialize renderer-owned ANSI. Every styled span is independently reset,
    /// and a final reset prevents bleed even when no spans were present.
    pub fn ansi(&self) -> String {
        let mut out = String::new();
        for span in &self.spans {
            let sgr = span.role.sgr();
            if sgr.is_empty() {
                out.push_str(span.text());
            } else {
                out.push_str(sgr);
                out.push_str(span.text());
                out.push_str(RESET);
            }
        }
        if !out.ends_with(RESET) {
            out.push_str(RESET);
        }
        out
    }

    pub fn render(&self, color: ResolvedColorMode) -> String {
        match color {
            ResolvedColorMode::Color => self.ansi(),
            ResolvedColorMode::Plain => self.plain(),
        }
    }

    pub fn truncate(&self, maximum_width: usize) -> Self {
        if console::measure_text_width(&self.plain()) <= maximum_width {
            return self.clone();
        }
        let content_width = maximum_width.saturating_sub(1);
        let mut width = 0;
        let mut truncated = Self::new();
        for span in &self.spans {
            let mut text = String::new();
            let mut finished = false;
            for character in span.text.chars() {
                if character == '\n' {
                    finished = true;
                    break;
                }
                let character_width = console::measure_text_width(&character.to_string());
                if width + character_width > content_width {
                    finished = true;
                    break;
                }
                width += character_width;
                text.push(character);
            }
            if !text.is_empty() {
                truncated.push(StyledSpan {
                    text,
                    role: span.role,
                });
            }
            if finished {
                break;
            }
        }
        if maximum_width > 0 {
            truncated.inline("…", StyleRole::Dim);
        }
        truncated
    }
}

impl From<StyledSpan> for StyledText {
    fn from(value: StyledSpan) -> Self {
        Self::from_span(value)
    }
}

/// Remove terminal controls while retaining safe Unicode.
pub fn sanitize_inline(input: &str) -> String {
    sanitize(input, false)
}

/// Remove terminal controls while retaining LF-delimited line structure.
pub fn sanitize_multiline(input: &str) -> String {
    sanitize(input, true)
}

fn sanitize(input: &str, multiline: bool) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == ESC {
            i = consume_escape(&chars, i);
            out.push(REPLACEMENT);
            continue;
        }
        if ch == '\u{009b}' {
            // C1 CSI
            i = consume_csi(&chars, i + 1);
            out.push(REPLACEMENT);
            continue;
        }
        if ch == '\u{009d}' {
            // C1 OSC
            i = consume_osc(&chars, i + 1);
            out.push(REPLACEMENT);
            continue;
        }
        if is_bidi_control(ch) || ch.is_control() {
            match ch {
                '\n' if multiline => out.push('\n'),
                '\n' | '\t' => out.push(' '),
                _ => out.push(REPLACEMENT),
            }
        } else {
            out.push(ch);
        }
        i += 1;
    }
    out
}

fn consume_escape(chars: &[char], at: usize) -> usize {
    match chars.get(at + 1) {
        Some('[') => consume_csi(chars, at + 2),
        Some(']') => consume_osc(chars, at + 2),
        Some(_) => (at + 2).min(chars.len()),
        None => chars.len(),
    }
}

fn consume_csi(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() {
        let c = chars[i] as u32;
        i += 1;
        if (0x40..=0x7e).contains(&c) {
            break;
        }
    }
    i
}

fn consume_osc(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() {
        if chars[i] == '\u{7}' {
            return i + 1;
        }
        if chars[i] == ESC && chars.get(i + 1) == Some(&'\\') {
            return i + 2;
        }
        i += 1;
    }
    i
}

fn is_bidi_control(ch: char) -> bool {
    matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolvedColorMode {
    Color,
    Plain,
}

impl ColorMode {
    pub fn resolve(self) -> ResolvedColorMode {
        self.resolve_with(|key| env::var(key).ok())
    }

    pub fn resolve_with(self, get: impl Fn(&str) -> Option<String>) -> ResolvedColorMode {
        match self {
            Self::Always => ResolvedColorMode::Color,
            Self::Never => ResolvedColorMode::Plain,
            Self::Auto => {
                if get("NO_COLOR").is_some() || get("TERM").as_deref() == Some("dumb") {
                    ResolvedColorMode::Plain
                } else {
                    ResolvedColorMode::Color
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum IconMode {
    Auto,
    Unicode,
    Ascii,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolvedIconMode {
    Unicode,
    Ascii,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PresentationOptions {
    pub color: ResolvedColorMode,
    /// Color capability for Television source rows. This may be downgraded
    /// independently when a TV build cannot combine ANSI with `display`.
    pub source_color: ResolvedColorMode,
    pub icons: ResolvedIconMode,
    pub compact: bool,
}

impl Default for PresentationOptions {
    fn default() -> Self {
        Self {
            color: ResolvedColorMode::Plain,
            source_color: ResolvedColorMode::Plain,
            icons: ResolvedIconMode::Ascii,
            compact: false,
        }
    }
}

impl PresentationOptions {
    pub fn resolve(color: ColorMode, icons: IconMode, columns: Option<u16>) -> Self {
        let color = color.resolve();
        Self {
            color,
            source_color: color,
            icons: icons.resolve(),
            compact: columns.is_some_and(|columns| columns < 100),
        }
    }
}

impl IconMode {
    pub fn resolve(self) -> ResolvedIconMode {
        self.resolve_with(|key| env::var(key).ok())
    }

    pub fn resolve_with(self, get: impl Fn(&str) -> Option<String>) -> ResolvedIconMode {
        match self {
            Self::Unicode => ResolvedIconMode::Unicode,
            Self::Ascii => ResolvedIconMode::Ascii,
            Self::None => ResolvedIconMode::None,
            Self::Auto => {
                if get("NO_ICONS").as_deref() == Some("1") || get("TERM").as_deref() == Some("dumb")
                {
                    return ResolvedIconMode::Ascii;
                }
                let locale = get("LC_ALL")
                    .or_else(|| get("LC_CTYPE"))
                    .or_else(|| get("LANG"));
                if locale.as_deref().is_some_and(unicode_locale) {
                    ResolvedIconMode::Unicode
                } else {
                    ResolvedIconMode::Ascii
                }
            }
        }
    }
}

fn unicode_locale(locale: &str) -> bool {
    let lower = locale.to_ascii_lowercase();
    lower.contains("utf-8") || lower.contains("utf8")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Icon {
    Standalone,
    Subfolder,
    Core,
    Docker,
    Test,
    Deploy,
    Module,
}

impl Icon {
    pub const fn render(self, mode: ResolvedIconMode) -> &'static str {
        match (mode, self) {
            (ResolvedIconMode::None, _) => "",
            (ResolvedIconMode::Unicode, Self::Standalone) => "▶",
            (ResolvedIconMode::Unicode, Self::Subfolder) => "📁",
            (ResolvedIconMode::Unicode, Self::Core) => "🔷",
            (ResolvedIconMode::Unicode, Self::Docker) => "🐳",
            (ResolvedIconMode::Unicode, Self::Test) => "🧪",
            (ResolvedIconMode::Unicode, Self::Deploy) => "🚀",
            (ResolvedIconMode::Unicode, Self::Module) => "📦",
            (ResolvedIconMode::Ascii, Self::Standalone) => "[recipe]",
            (ResolvedIconMode::Ascii, Self::Subfolder) => "[dir]",
            (ResolvedIconMode::Ascii, Self::Core) => "[core]",
            (ResolvedIconMode::Ascii, Self::Docker) => "[docker]",
            (ResolvedIconMode::Ascii, Self::Test) => "[test]",
            (ResolvedIconMode::Ascii, Self::Deploy) => "[deploy]",
            (ResolvedIconMode::Ascii, Self::Module) => "[mod]",
        }
    }

    pub fn for_module(module: Option<&str>, modular_project: bool) -> Self {
        match module {
            Some("docker") => Self::Docker,
            Some("test" | "testing") => Self::Test,
            Some("deploy" | "deployment") => Self::Deploy,
            Some(_) => Self::Module,
            None if modular_project => Self::Core,
            None => Self::Standalone,
        }
    }
}

/// Error returned when optional renderer output contains anything except text,
/// LF/TAB, and syntactically valid SGR control sequences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnsiValidationError {
    offset: usize,
}

impl AnsiValidationError {
    pub const fn offset(&self) -> usize {
        self.offset
    }
}

impl fmt::Display for AnsiValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "non-SGR terminal control at byte {}", self.offset)
    }
}

impl std::error::Error for AnsiValidationError {}

/// Validate and return highlighter output containing only safe text and SGR.
pub fn validate_sgr_only(input: &str) -> Result<&str, AnsiValidationError> {
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            0x1b => {
                let start = i;
                if bytes.get(i + 1) != Some(&b'[') {
                    return Err(AnsiValidationError { offset: start });
                }
                i += 2;
                let params = i;
                while bytes
                    .get(i)
                    .is_some_and(|b| b.is_ascii_digit() || matches!(b, b';' | b':'))
                {
                    i += 1;
                }
                if bytes.get(i) != Some(&b'm') || !valid_sgr_params(&input[params..i]) {
                    return Err(AnsiValidationError { offset: start });
                }
                i += 1;
            }
            b'\n' | b'\t' => i += 1,
            b if b < 0x20 || b == 0x7f => return Err(AnsiValidationError { offset: i }),
            _ => {
                let ch = input[i..].chars().next().expect("valid UTF-8 boundary");
                if is_bidi_control(ch) || ch as u32 >= 0x80 && ch.is_control() {
                    return Err(AnsiValidationError { offset: i });
                }
                i += ch.len_utf8();
            }
        }
    }
    Ok(input)
}

fn valid_sgr_params(params: &str) -> bool {
    if params.is_empty() {
        return true;
    }
    let Some(values) = params
        .split([';', ':'])
        .map(|part| part.parse::<u16>().ok())
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let mut i = 0;
    while i < values.len() {
        let code = values[i];
        if matches!(code, 38 | 48 | 58) {
            match values.get(i + 1) {
                Some(5) if values.get(i + 2).is_some_and(|value| *value <= 255) => i += 3,
                Some(2)
                    if values
                        .get(i + 2..i + 5)
                        .is_some_and(|rgb| rgb.iter().all(|value| *value <= 255)) =>
                {
                    i += 5
                }
                _ => return false,
            }
        } else if is_basic_sgr(code) {
            i += 1;
        } else {
            return false;
        }
    }
    true
}

fn is_basic_sgr(code: u16) -> bool {
    matches!(code, 0..=9 | 21..=29 | 30..=37 | 39 | 40..=47 | 49 | 53 | 55 | 59 | 90..=107)
}
