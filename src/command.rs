//! Safe `just` command planning. Plans are OS argument vectors, never shell text.

use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use crate::{
    Error, Result,
    invocation::Invocation,
    model::Recipe,
    parameters::{CollectedParameters, ParameterValue},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPlan {
    pub program: PathBuf,
    pub cwd: PathBuf,
    pub args: Vec<OsString>,
    pub redacted_args: Vec<String>,
    /// True only when an argument value emitted by this plan came from a
    /// parameter explicitly configured as secret.
    pub contains_secret: bool,
}

impl CommandPlan {
    pub fn display_redacted(&self) -> String {
        std::iter::once(self.program.to_string_lossy().into_owned())
            .chain(self.redacted_args.iter().cloned())
            .map(|part| shell_escape_display(&part))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Render an executable zsh command for shell-history integration.
    ///
    /// Secret-bearing plans are deliberately omitted rather than redacted:
    /// a redacted command would not be a faithful, repeatable history entry.
    pub fn history_command_zsh(&self) -> Result<Option<String>> {
        if self.contains_secret {
            return Ok(None);
        }
        let words = std::iter::once(self.program.as_os_str())
            .chain(self.args.iter().map(OsString::as_os_str))
            .map(zsh_quote)
            .collect::<Result<Vec<_>>>()?;
        Ok(Some(words.join(" ")))
    }
}

pub fn build_plan(
    invocation: &Invocation,
    recipe: &Recipe,
    values: &CollectedParameters,
) -> Result<CommandPlan> {
    let mut args = Vec::new();
    let mut redacted = Vec::new();
    let mut contains_secret = false;
    let mut push = |arg: OsString, shown: String, secret: bool| {
        args.push(arg);
        redacted.push(shown);
        contains_secret |= secret;
    };
    if invocation.dry_run {
        push("--dry-run".into(), "--dry-run".into(), false);
    }
    if let Some(path) = &invocation.justfile {
        push("--justfile".into(), "--justfile".into(), false);
        push(
            path.as_os_str().into(),
            path.to_string_lossy().into_owned(),
            false,
        );
    }
    push(
        recipe.namepath.clone().into(),
        recipe.namepath.clone(),
        false,
    );

    // Flags are independent of positional default materialization.
    for parameter in recipe.parameters.iter().filter(|parameter| parameter.flag) {
        let Some(value) = values.value(&parameter.name) else {
            continue;
        };
        if matches!(value, ParameterValue::Flag(false) | ParameterValue::Omitted)
            || matches!(value, ParameterValue::Many(items) if items.is_empty())
        {
            continue;
        }
        let flag = flag_spelling(
            parameter.long.as_deref(),
            parameter.short.as_deref(),
            &parameter.name,
        );
        let secret = values.is_secret(&parameter.name);
        push(flag.clone().into(), flag, secret);
        match value {
            ParameterValue::Scalar(value) => {
                push(value.clone(), values.redact(&parameter.name, value), secret)
            }
            ParameterValue::Many(items) => {
                for item in items {
                    push(item.clone(), values.redact(&parameter.name, item), secret);
                }
            }
            ParameterValue::Flag(true) => {}
            _ => {}
        }
    }

    let positional: Vec<_> = recipe
        .parameters
        .iter()
        .filter(|parameter| !parameter.flag)
        .collect();
    let last_required =
        positional
            .iter()
            .rposition(|parameter| match values.value(&parameter.name) {
                Some(ParameterValue::Scalar(value)) => parameter
                    .default
                    .as_deref()
                    .is_none_or(|default| value != OsStr::new(default)),
                Some(ParameterValue::Many(items)) => !items.is_empty(),
                Some(ParameterValue::Flag(true)) => true,
                _ => false,
            });
    if let Some(last) = last_required {
        for parameter in &positional[..=last] {
            match values.value(&parameter.name) {
                Some(ParameterValue::Scalar(value)) => push(
                    value.clone(),
                    values.redact(&parameter.name, value),
                    values.is_secret(&parameter.name),
                ),
                Some(ParameterValue::Many(items)) => {
                    for item in items {
                        push(
                            item.clone(),
                            values.redact(&parameter.name, item),
                            values.is_secret(&parameter.name),
                        );
                    }
                }
                Some(ParameterValue::Omitted) | None => {
                    let default = parameter.default.as_ref().ok_or_else(|| {
                        if parameter.default_expression.is_some() {
                            Error::Message(format!(
                                "parameter `{}` uses a non-literal just default; enter an explicit value because a later positional value was supplied",
                                parameter.name
                            ))
                        } else {
                            Error::Message(format!(
                                "missing required parameter `{}`",
                                parameter.name
                            ))
                        }
                    })?;
                    push(
                        default.into(),
                        if values.is_secret(&parameter.name) {
                            "[REDACTED]".into()
                        } else {
                            default.clone()
                        },
                        values.is_secret(&parameter.name),
                    );
                }
                Some(ParameterValue::Flag(_)) => {
                    return Err(Error::Message(format!(
                        "invalid positional value for `{}`",
                        parameter.name
                    )));
                }
            }
        }
    }
    Ok(CommandPlan {
        program: invocation.just_binary.clone(),
        cwd: invocation.cwd.clone(),
        args,
        redacted_args: redacted,
        contains_secret,
    })
}

#[cfg(unix)]
fn zsh_quote(value: &OsStr) -> Result<String> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Ok("''".into());
    }
    if bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || b"_./:@%+=,-".contains(byte))
    {
        return Ok(String::from_utf8(bytes.to_vec()).expect("safe bytes are ASCII"));
    }
    if !bytes.contains(&0) && !bytes.contains(&b'\n') && !bytes.contains(&b'\r') {
        if let Ok(text) = std::str::from_utf8(bytes) {
            return Ok(format!("'{}'", text.replace('\'', "'\\''")));
        }
    }

    // ANSI-C quoting is executable zsh syntax and can represent every non-NUL
    // Unix argv byte. Emit non-ASCII bytes as fixed-width hex escapes in their
    // own quoted segment so a following hex digit can never be consumed.
    let mut output = String::from("$'");
    for byte in bytes {
        match byte {
            b'\0' => {
                return Err(Error::Message(
                    "cannot serialize a NUL byte in a zsh command".into(),
                ));
            }
            b'\\' => output.push_str("\\\\"),
            b'\'' => output.push_str("\\'"),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            0x20..=0x7e => output.push(char::from(*byte)),
            byte => {
                use std::fmt::Write;
                write!(output, "\\x{byte:02x}'$'").expect("writing to a String cannot fail");
            }
        }
    }
    output.push('\'');
    Ok(output)
}

#[cfg(not(unix))]
fn zsh_quote(_value: &OsStr) -> Result<String> {
    Err(Error::Message(
        "zsh command serialization is only supported on Unix".into(),
    ))
}

fn flag_spelling(long: Option<&str>, short: Option<&str>, name: &str) -> String {
    if let Some(long) = long {
        if long.starts_with('-') {
            long.into()
        } else {
            format!("--{long}")
        }
    } else if let Some(short) = short {
        if short.starts_with('-') {
            short.into()
        } else {
            format!("-{short}")
        }
    } else {
        format!("--{}", name.replace('_', "-"))
    }
}

fn shell_escape_display(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:@%+=,-".contains(c))
    {
        return value.into();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
