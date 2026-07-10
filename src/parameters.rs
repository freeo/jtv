//! Interactive parameter state machine.

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::Path,
};

use crate::{
    Error, Result,
    config::{Config, ParameterConfig},
    model::{ParameterKind, Recipe},
    picker::{PathKind, Picker},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterValue {
    Omitted,
    Scalar(OsString),
    Many(Vec<OsString>),
    Flag(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectedParameters {
    pub values: BTreeMap<String, ParameterValue>,
    secret_names: Vec<String>,
}

impl CollectedParameters {
    pub fn new(values: BTreeMap<String, ParameterValue>, secret_names: Vec<String>) -> Self {
        Self {
            values,
            secret_names,
        }
    }

    pub fn value(&self, name: &str) -> Option<&ParameterValue> {
        self.values.get(name)
    }
    pub fn is_secret(&self, name: &str) -> bool {
        self.secret_names.iter().any(|n| n == name)
    }
    pub fn redact(&self, name: &str, value: &OsStr) -> String {
        if self.is_secret(name) {
            "[REDACTED]".into()
        } else {
            value.to_string_lossy().into_owned()
        }
    }
}

pub trait Prompter {
    fn input(&mut self, label: &str, default: Option<&str>, secret: bool)
    -> Result<Option<String>>;
    fn variadic(
        &mut self,
        label: &str,
        minimum: usize,
        secret: bool,
    ) -> Result<Option<Vec<String>>>;
}

#[derive(Default)]
pub struct DialoguerPrompter;

impl Prompter for DialoguerPrompter {
    fn input(
        &mut self,
        label: &str,
        default: Option<&str>,
        secret: bool,
    ) -> Result<Option<String>> {
        let result = if secret {
            dialoguer::Password::new()
                .with_prompt(label)
                .allow_empty_password(default.is_some())
                .interact()
        } else {
            let mut prompt = dialoguer::Input::<String>::new().with_prompt(label);
            if let Some(default) = default {
                prompt = prompt.default(default.to_owned());
            }
            prompt.allow_empty(true).interact_text()
        };
        result
            .map(Some)
            .map_err(|error| Error::Message(format!("input failed: {error}")))
    }

    fn variadic(
        &mut self,
        label: &str,
        minimum: usize,
        secret: bool,
    ) -> Result<Option<Vec<String>>> {
        let mut values = Vec::new();
        loop {
            let prompt = format!("{label} (value {}, empty to finish)", values.len() + 1);
            let Some(value) = self.input(&prompt, None, secret)? else {
                return Ok(None);
            };
            if value.is_empty() {
                if values.len() >= minimum {
                    return Ok(Some(values));
                }
                continue;
            }
            values.push(value);
        }
    }
}

pub fn collect<P: Prompter, K: Picker>(
    recipe: &Recipe,
    config: &Config,
    cwd: &Path,
    prompts: &mut P,
    picker: &mut K,
) -> Result<CollectedParameters> {
    let mut values = BTreeMap::new();
    let mut secret_names = Vec::new();
    for parameter in &recipe.parameters {
        let metadata = config.parameter(&recipe.namepath, &parameter.name);
        let secret = matches!(metadata, Some(ParameterConfig::Secret));
        if secret {
            secret_names.push(parameter.name.clone());
        }
        let label = parameter.help.as_deref().unwrap_or(&parameter.name);
        let value = if parameter.flag && parameter.value.is_none() {
            let choices = vec!["true".into(), "false".into()];
            let answer = picker.choose(label, &choices)?.ok_or(Error::Cancelled)?;
            ParameterValue::Flag(answer == "true")
        } else if !matches!(parameter.kind, ParameterKind::Singular) {
            let minimum = usize::from(matches!(parameter.kind, ParameterKind::Plus));
            let answer = prompts
                .variadic(label, minimum, secret)?
                .ok_or(Error::Cancelled)?;
            if answer.len() < minimum {
                return Err(Error::Message(format!(
                    "parameter `{}` requires at least {minimum} value(s)",
                    parameter.name
                )));
            }
            ParameterValue::Many(answer.into_iter().map(OsString::from).collect())
        } else {
            let answer: OsString = match metadata {
                Some(ParameterConfig::Choice { values: choices }) => {
                    if choices.is_empty() {
                        return Err(Error::Message(format!(
                            "choice parameter `{}` has no values",
                            parameter.name
                        )));
                    }
                    picker
                        .choose(label, choices)?
                        .ok_or(Error::Cancelled)?
                        .into()
                }
                Some(ParameterConfig::Boolean) => picker
                    .choose(label, &["true".into(), "false".into()])?
                    .ok_or(Error::Cancelled)?
                    .into(),
                Some(ParameterConfig::File) | Some(ParameterConfig::Directory) => {
                    let kind = if matches!(metadata, Some(ParameterConfig::File)) {
                        PathKind::File
                    } else {
                        PathKind::Directory
                    };
                    picker
                        .choose_path(label, cwd, kind)?
                        .ok_or(Error::Cancelled)?
                        .into_os_string()
                }
                _ => prompts
                    .input(label, parameter.default.as_deref(), secret)?
                    .ok_or(Error::Cancelled)?
                    .into(),
            };
            if answer.is_empty() {
                if let Some(default) = &parameter.default {
                    ParameterValue::Scalar(default.into())
                } else if parameter.default_expression.is_some() {
                    ParameterValue::Omitted
                } else {
                    return Err(Error::Message(format!(
                        "parameter `{}` is required",
                        parameter.name
                    )));
                }
            } else {
                ParameterValue::Scalar(answer)
            }
        };
        values.insert(parameter.name.clone(), value);
    }

    let positional: Vec<_> = recipe
        .parameters
        .iter()
        .filter(|parameter| !parameter.flag)
        .collect();
    let last_explicit =
        positional
            .iter()
            .rposition(|parameter| match values.get(&parameter.name) {
                Some(ParameterValue::Scalar(value)) => parameter
                    .default
                    .as_deref()
                    .is_none_or(|default| value != OsStr::new(default)),
                Some(ParameterValue::Many(items)) => !items.is_empty(),
                _ => false,
            });
    if let Some(last) = last_explicit {
        for parameter in &positional[..last] {
            if !matches!(values.get(&parameter.name), Some(ParameterValue::Omitted)) {
                continue;
            }
            let label = format!(
                "{} (explicit value required before a later positional argument)",
                parameter.help.as_deref().unwrap_or(&parameter.name)
            );
            let secret = secret_names.iter().any(|name| name == &parameter.name);
            let answer = prompts
                .input(&label, None, secret)?
                .ok_or(Error::Cancelled)?;
            if answer.is_empty() {
                return Err(Error::Message(format!(
                    "parameter `{}` needs an explicit value before a later positional argument",
                    parameter.name
                )));
            }
            values.insert(
                parameter.name.clone(),
                ParameterValue::Scalar(answer.into()),
            );
        }
    }
    Ok(CollectedParameters {
        values,
        secret_names,
    })
}
