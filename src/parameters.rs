//! Interactive parameter state machine.

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::Path,
};

use crate::{
    Error, Result,
    config::{Config, ParameterConfig},
    input::{EditAction, LineBuffer},
    model::{ParameterKind, Recipe},
    picker::{PathKind, Picker},
    presentation::ResolvedColorMode,
};
use dialoguer::theme::{ColorfulTheme, SimpleTheme, Theme};

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

    fn input_with_completion(
        &mut self,
        label: &str,
        default: Option<&str>,
        cwd: &Path,
        picker: &mut dyn Picker,
    ) -> Result<Option<String>> {
        let _ = (cwd, picker);
        self.input(label, default, false)
    }

    fn variadic_with_completion(
        &mut self,
        label: &str,
        minimum: usize,
        cwd: &Path,
        picker: &mut dyn Picker,
    ) -> Result<Option<Vec<String>>> {
        let _ = (cwd, picker);
        self.variadic(label, minimum, false)
    }
}

pub struct DialoguerPrompter {
    color: ResolvedColorMode,
}

impl Default for DialoguerPrompter {
    fn default() -> Self {
        Self {
            color: ResolvedColorMode::Plain,
        }
    }
}

impl DialoguerPrompter {
    pub const fn new(color: ResolvedColorMode) -> Self {
        Self { color }
    }
}

pub fn dialoguer_theme(color: ResolvedColorMode) -> Box<dyn Theme> {
    match color {
        ResolvedColorMode::Plain => Box::new(SimpleTheme),
        ResolvedColorMode::Color => Box::new(ColorfulTheme {
            prompt_style: console::Style::new().for_stderr().yellow().bold(),
            defaults_style: console::Style::new().for_stderr().green(),
            values_style: console::Style::new().for_stderr().cyan(),
            ..ColorfulTheme::default()
        }),
    }
}

impl Prompter for DialoguerPrompter {
    fn input(
        &mut self,
        label: &str,
        default: Option<&str>,
        secret: bool,
    ) -> Result<Option<String>> {
        let theme = dialoguer_theme(self.color);
        let result = if secret {
            dialoguer::Password::with_theme(theme.as_ref())
                .with_prompt(label)
                .allow_empty_password(default.is_some())
                .interact()
        } else {
            let mut prompt =
                dialoguer::Input::<String>::with_theme(theme.as_ref()).with_prompt(label);
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

    fn input_with_completion(
        &mut self,
        label: &str,
        default: Option<&str>,
        cwd: &Path,
        picker: &mut dyn Picker,
    ) -> Result<Option<String>> {
        use console::{Term, measure_text_width};

        let term = Term::stderr();
        let mut line = LineBuffer::new("");
        let prompt = match (self.color, default) {
            (ResolvedColorMode::Color, Some(default)) => format!(
                "{} {} {} ",
                console::style(label).yellow().bold(),
                console::style(format!("[{default}]")).green(),
                console::style("[TAB files] ›").dim()
            ),
            (ResolvedColorMode::Color, None) => format!(
                "{} {} ",
                console::style(label).yellow().bold(),
                console::style("[TAB files] ›").dim()
            ),
            (ResolvedColorMode::Plain, Some(default)) => {
                format!("{label} [{default}] [TAB files] > ")
            }
            (ResolvedColorMode::Plain, None) => format!("{label} [TAB files] > "),
        };

        loop {
            redraw_line(&term, &prompt, &line, measure_text_width)?;
            let key = term
                .read_key()
                .map_err(|error| Error::Message(format!("input failed: {error}")))?;
            match line.apply(key) {
                EditAction::Continue => {}
                EditAction::Submit(value) => {
                    term.write_line("").map_err(input_error)?;
                    return Ok(Some(if value.is_empty() {
                        default.unwrap_or_default().to_owned()
                    } else {
                        value
                    }));
                }
                EditAction::Cancel => {
                    term.write_line("").map_err(input_error)?;
                    return Ok(None);
                }
                EditAction::Browse { buffer, .. } => {
                    // `read_key` restores terminal state before returning. Clear the
                    // prompt so no raw-mode guard or terminal lock crosses into TV.
                    term.clear_line().map_err(input_error)?;
                    term.flush().map_err(input_error)?;
                    if let Some(selected) =
                        picker.complete_path("Select file or directory", cwd, &buffer)?
                    {
                        line.replace(&selected.to_string_lossy());
                    }
                }
            }
        }
    }

    fn variadic_with_completion(
        &mut self,
        label: &str,
        minimum: usize,
        cwd: &Path,
        picker: &mut dyn Picker,
    ) -> Result<Option<Vec<String>>> {
        let mut values = Vec::new();
        loop {
            let prompt = format!("{label} (value {}, empty to finish)", values.len() + 1);
            let Some(value) = self.input_with_completion(&prompt, None, cwd, picker)? else {
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

fn input_error(error: std::io::Error) -> Error {
    Error::Message(format!("input failed: {error}"))
}

fn redraw_line(
    term: &console::Term,
    prompt: &str,
    line: &LineBuffer,
    width: impl Fn(&str) -> usize,
) -> Result<()> {
    term.clear_line().map_err(input_error)?;
    term.write_str(prompt).map_err(input_error)?;
    term.write_str(&line.value()).map_err(input_error)?;
    let suffix_width = width(&line.suffix());
    if suffix_width > 0 {
        term.move_cursor_left(suffix_width).map_err(input_error)?;
    }
    term.flush().map_err(input_error)
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
    let parameter_count = recipe.parameters.len();
    for (parameter_index, parameter) in recipe.parameters.iter().enumerate() {
        let metadata = config.parameter(&recipe.namepath, &parameter.name);
        let secret = matches!(metadata, Some(ParameterConfig::Secret));
        if secret {
            secret_names.push(parameter.name.clone());
        }
        let label = parameter_label(parameter, metadata, parameter_index + 1, parameter_count);
        let value = if parameter.flag && parameter.value.is_some() {
            let choices = vec!["true".into(), "false".into()];
            let picker_label = picker_label(
                parameter,
                parameter_index + 1,
                parameter_count,
                "Choose",
                "boolean flag",
            );
            let answer = picker
                .choose(&picker_label, &choices)?
                .ok_or(Error::Cancelled)?;
            ParameterValue::Flag(answer == "true")
        } else if !matches!(parameter.kind, ParameterKind::Singular) {
            let minimum = usize::from(matches!(parameter.kind, ParameterKind::Plus));
            let answer = if secret {
                prompts.variadic(&label, minimum, true)?
            } else {
                prompts.variadic_with_completion(&label, minimum, cwd, picker)?
            }
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
                        .choose(
                            &picker_label(
                                parameter,
                                parameter_index + 1,
                                parameter_count,
                                "Choose",
                                "choice",
                            ),
                            choices,
                        )?
                        .ok_or(Error::Cancelled)?
                        .into()
                }
                Some(ParameterConfig::Boolean) => picker
                    .choose(
                        &picker_label(
                            parameter,
                            parameter_index + 1,
                            parameter_count,
                            "Choose",
                            "boolean",
                        ),
                        &["true".into(), "false".into()],
                    )?
                    .ok_or(Error::Cancelled)?
                    .into(),
                Some(ParameterConfig::File) | Some(ParameterConfig::Directory) => {
                    let kind = if matches!(metadata, Some(ParameterConfig::File)) {
                        PathKind::File
                    } else {
                        PathKind::Directory
                    };
                    picker
                        .choose_path(
                            &picker_label(
                                parameter,
                                parameter_index + 1,
                                parameter_count,
                                "Select",
                                if kind == PathKind::File {
                                    "file"
                                } else {
                                    "directory"
                                },
                            ),
                            cwd,
                            kind,
                        )?
                        .ok_or(Error::Cancelled)?
                        .into_os_string()
                }
                _ if secret => prompts
                    .input(&label, parameter.default.as_deref(), true)?
                    .ok_or(Error::Cancelled)?
                    .into(),
                _ => prompts
                    .input_with_completion(&label, parameter.default.as_deref(), cwd, picker)?
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
            let answer = if secret {
                prompts.input(&label, None, true)?
            } else {
                prompts.input_with_completion(&label, None, cwd, picker)?
            }
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

fn parameter_label(
    parameter: &crate::model::Parameter,
    metadata: Option<&ParameterConfig>,
    current: usize,
    total: usize,
) -> String {
    let kind = match metadata {
        Some(ParameterConfig::Secret) => "secret",
        Some(ParameterConfig::Choice { .. }) => "choice",
        Some(ParameterConfig::Boolean) => "boolean",
        Some(ParameterConfig::File) => "file",
        Some(ParameterConfig::Directory) => "directory",
        Some(ParameterConfig::String) | None => "text",
    };
    let requirement = if let Some(default) = &parameter.default {
        if matches!(metadata, Some(ParameterConfig::Secret)) {
            "default: [REDACTED]".to_owned()
        } else {
            format!("default: {default}")
        }
    } else if parameter.default_expression.is_some() {
        "just default".into()
    } else if parameter.flag && parameter.value.is_some() {
        "optional flag".into()
    } else {
        "required".into()
    };
    let help = parameter
        .help
        .as_deref()
        .map(|help| format!(" — {help}"))
        .unwrap_or_default();
    format!(
        "[{current}/{total}] {} ({kind}, {requirement}){help}",
        parameter.name
    )
}

fn picker_label(
    parameter: &crate::model::Parameter,
    current: usize,
    total: usize,
    verb: &str,
    kind: &str,
) -> String {
    format!("[{current}/{total}] {verb} {} — {kind}", parameter.name)
}
