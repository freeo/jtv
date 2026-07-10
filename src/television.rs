//! Television process integration and row protocol.

use std::{
    fmt::Write as _,
    path::Path,
    process::{Command, ExitStatus, Stdio},
};

use semver::Version;

use crate::{
    Error, Result,
    channel::CHANNEL_NAME,
    model::Recipe,
    session::{SESSION_ENV, SessionFile, SessionState, validate_id},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRow {
    pub id: String,
    pub display: String,
    pub search: String,
}

impl SourceRow {
    pub fn encode(&self) -> Result<String> {
        validate_id(&self.id)?;
        Ok(format!(
            "{}\t{}\t{}",
            self.id,
            sanitize_field(&self.display),
            sanitize_field(&self.search)
        ))
    }
}

pub fn rows(state: &SessionState) -> Vec<SourceRow> {
    state
        .selections
        .iter()
        .filter_map(|(id, namepath)| {
            state.project.recipe(namepath).map(|recipe| SourceRow {
                id: id.clone(),
                display: signature(recipe),
                search: searchable(recipe),
            })
        })
        .collect()
}

pub fn source_output(state: &SessionState) -> Result<String> {
    let mut output = String::new();
    for row in rows(state) {
        writeln!(output, "{}", row.encode()?).expect("String writes cannot fail");
    }
    Ok(output)
}

pub fn preview(state: &SessionState, id: &str) -> Result<String> {
    let recipe = state
        .project
        .recipe(state.resolve(id)?)
        .ok_or_else(|| Error::InvalidSelection(id.into()))?;
    let mut output = String::new();
    writeln!(output, "{}", signature(recipe)).ok();
    if let Some(doc) = &recipe.doc {
        writeln!(output, "\n{}", sanitize_preview(doc)).ok();
    }
    if !recipe.dependencies.is_empty() {
        writeln!(output, "\nDependencies: {}", recipe.dependencies.join(", ")).ok();
    }
    if !recipe.body.is_empty() {
        writeln!(
            output,
            "\n{}",
            recipe
                .body
                .iter()
                .map(|line| sanitize_preview(line))
                .collect::<Vec<_>>()
                .join("\n")
        )
        .ok();
    }
    Ok(output)
}

pub fn launch(
    tv_binary: &Path,
    cable_dir: &Path,
    cwd: &Path,
    session: &SessionFile,
) -> Result<ExitStatus> {
    command(tv_binary, cable_dir, cwd, session)
        .status()
        .map_err(|source| Error::Spawn {
            program: tv_binary.display().to_string(),
            source,
        })
}

/// Construct the exact Television child command, exposed for orchestration and contract tests.
pub fn command(tv_binary: &Path, cable_dir: &Path, cwd: &Path, session: &SessionFile) -> Command {
    let mut command = Command::new(tv_binary);
    command
        .arg(CHANNEL_NAME)
        .arg(cwd)
        .arg("--cable-dir")
        .arg(cable_dir)
        .arg("--no-remote")
        .env(SESSION_ENV, session.path())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

pub fn ensure_success(status: ExitStatus) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        Err(Error::ProgramFailed {
            program: "tv".into(),
            status: status.code().unwrap_or(130),
            stderr: String::new(),
        })
    }
}

/// Probe a Television executable and return its semantic version.
pub fn version(tv_binary: &Path) -> Result<Version> {
    let output = Command::new(tv_binary)
        .arg("--version")
        .output()
        .map_err(|source| Error::Spawn {
            program: tv_binary.display().to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(Error::ProgramFailed {
            program: tv_binary.display().to_string(),
            status: output.status.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let value = text
        .split_whitespace()
        .find(|word| word.as_bytes().first().is_some_and(u8::is_ascii_digit))
        .ok_or_else(|| Error::Message(format!("unrecognized Television version output: {text}")))?;
    Version::parse(value).map_err(|error| {
        Error::Message(format!(
            "unrecognized Television version `{value}`: {error}"
        ))
    })
}

/// Compatibility floor validated by this implementation.
pub fn version_is_supported(version: &Version) -> bool {
    version >= &Version::new(0, 15, 9)
}

fn signature(recipe: &Recipe) -> String {
    let mut text = recipe.namepath.clone();
    for parameter in &recipe.parameters {
        text.push(' ');
        if parameter.has_default() {
            text.push('[');
        }
        text.push_str(&parameter.name);
        if let Some(default) = &parameter.default {
            text.push('=');
            text.push_str(default);
            text.push(']');
        } else if parameter.default_expression.is_some() {
            text.push_str("=<just default>]");
        }
    }
    sanitize_field(&text)
}

fn searchable(recipe: &Recipe) -> String {
    sanitize_field(
        &[
            Some(recipe.namepath.as_str()),
            recipe.doc.as_deref(),
            recipe.group.as_deref(),
            recipe.module.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" "),
    )
}

pub fn sanitize_field(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character == '\t' || character == '\n' || character == '\r' || character.is_control()
            {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn sanitize_preview(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character == '\n' || *character == '\t' || !character.is_control())
        .collect()
}
