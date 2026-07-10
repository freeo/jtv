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
}

impl CommandPlan {
    pub fn display_redacted(&self) -> String {
        std::iter::once(self.program.to_string_lossy().into_owned())
            .chain(self.redacted_args.iter().cloned())
            .map(|part| shell_escape_display(&part))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub fn build_plan(
    invocation: &Invocation,
    recipe: &Recipe,
    values: &CollectedParameters,
) -> Result<CommandPlan> {
    let mut args = Vec::new();
    let mut redacted = Vec::new();
    let mut push = |arg: OsString, shown: String| {
        args.push(arg);
        redacted.push(shown);
    };
    if invocation.dry_run {
        push("--dry-run".into(), "--dry-run".into());
    }
    if let Some(path) = &invocation.justfile {
        push("--justfile".into(), "--justfile".into());
        push(path.as_os_str().into(), path.to_string_lossy().into_owned());
    }
    push(recipe.namepath.clone().into(), recipe.namepath.clone());

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
        push(flag.clone().into(), flag);
        match value {
            ParameterValue::Scalar(value) => {
                push(value.clone(), values.redact(&parameter.name, value))
            }
            ParameterValue::Many(items) => {
                for item in items {
                    push(item.clone(), values.redact(&parameter.name, item));
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
                Some(ParameterValue::Scalar(value)) => {
                    push(value.clone(), values.redact(&parameter.name, value))
                }
                Some(ParameterValue::Many(items)) => {
                    for item in items {
                        push(item.clone(), values.redact(&parameter.name, item));
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
    })
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
