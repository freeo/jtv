use jtv::{
    Result,
    config::Config,
    model::{Parameter, ParameterKind, Recipe},
    parameters::{ParameterValue, Prompter, collect},
    picker::{PathKind, Picker},
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

#[derive(Default)]
struct Fake {
    inputs: VecDeque<Option<String>>,
    many: VecDeque<Option<Vec<String>>>,
    choices: VecDeque<Option<String>>,
    paths: VecDeque<Option<PathBuf>>,
}
impl Prompter for Fake {
    fn input(&mut self, _: &str, _: Option<&str>, _: bool) -> Result<Option<String>> {
        Ok(self.inputs.pop_front().unwrap())
    }
    fn variadic(&mut self, _: &str, _: usize, _: bool) -> Result<Option<Vec<String>>> {
        Ok(self.many.pop_front().unwrap())
    }
}
impl Picker for Fake {
    fn choose(&mut self, _: &str, _: &[String]) -> Result<Option<String>> {
        Ok(self.choices.pop_front().unwrap())
    }
    fn choose_path(&mut self, _: &str, _: &Path, _: PathKind) -> Result<Option<PathBuf>> {
        Ok(self.paths.pop_front().unwrap())
    }
}
fn param(name: &str, default: Option<&str>, kind: ParameterKind, flag: bool) -> Parameter {
    Parameter {
        name: name.into(),
        default: default.map(Into::into),
        default_expression: None,
        kind,
        flag,
        long: None,
        short: None,
        help: None,
        value: None,
        pattern: None,
    }
}

#[test]
fn collects_defaults_flags_variadics_and_redacts_secrets() {
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![
            param("target", Some("dev"), ParameterKind::Singular, false),
            param("token", None, ParameterKind::Singular, false),
            param("verbose", None, ParameterKind::Singular, true),
            param("rest", None, ParameterKind::Star, false),
        ],
        ..Recipe::default()
    };
    let config: Config = toml::from_str("[recipes.r.params.token]\ntype='secret'").unwrap();
    let mut prompts = Fake {
        inputs: [Some("".into()), Some("s;$(bad)".into())].into(),
        many: [Some(vec!["a b".into(), "λ".into()])].into(),
        ..Fake::default()
    };
    let mut picker = Fake {
        choices: [Some("true".into())].into(),
        ..Fake::default()
    };
    let got = collect(&recipe, &config, Path::new("."), &mut prompts, &mut picker).unwrap();
    assert_eq!(
        got.value("target"),
        Some(&ParameterValue::Scalar("dev".into()))
    );
    assert_eq!(got.value("verbose"), Some(&ParameterValue::Flag(true)));
    assert_eq!(
        got.redact("token", std::ffi::OsStr::new("s;$(bad)")),
        "[REDACTED]"
    );
}

#[test]
fn cancellation_and_plus_cardinality_prevent_execution_planning() {
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![param("items", None, ParameterKind::Plus, false)],
        ..Recipe::default()
    };
    let mut prompts = Fake {
        many: [Some(vec![])].into(),
        ..Fake::default()
    };
    let error = collect(
        &recipe,
        &Config::default(),
        Path::new("."),
        &mut prompts,
        &mut Fake::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("at least 1"));
}

#[test]
fn materializes_expression_default_when_a_later_positional_is_supplied() {
    let mut first = param("first", None, ParameterKind::Singular, false);
    first.default_expression = Some("env_var('TARGET')".into());
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![first, param("second", None, ParameterKind::Singular, false)],
        ..Recipe::default()
    };
    let mut prompts = Fake {
        inputs: [
            Some("".into()),
            Some("later".into()),
            Some("resolved".into()),
        ]
        .into(),
        ..Fake::default()
    };
    let collected = collect(
        &recipe,
        &Config::default(),
        Path::new("."),
        &mut prompts,
        &mut Fake::default(),
    )
    .unwrap();
    assert_eq!(
        collected.value("first"),
        Some(&ParameterValue::Scalar("resolved".into()))
    );
    assert_eq!(
        collected.value("second"),
        Some(&ParameterValue::Scalar("later".into()))
    );
}

#[test]
fn collects_every_configured_parameter_type() {
    let recipe = Recipe {
        name: "deploy".into(),
        namepath: "deploy".into(),
        parameters: vec![
            param("text", None, ParameterKind::Singular, false),
            param("environment", None, ParameterKind::Singular, false),
            param("confirm", None, ParameterKind::Singular, false),
            param("manifest", None, ParameterKind::Singular, false),
            param("directory", None, ParameterKind::Singular, false),
        ],
        ..Recipe::default()
    };
    let config: Config = toml::from_str(
        r#"
[recipes.deploy.params.text]
type = "string"
[recipes.deploy.params.environment]
type = "choice"
values = ["dev", "prod"]
[recipes.deploy.params.confirm]
type = "boolean"
[recipes.deploy.params.manifest]
type = "file"
[recipes.deploy.params.directory]
type = "directory"
"#,
    )
    .unwrap();
    let mut prompts = Fake {
        inputs: [Some("literal".into())].into(),
        ..Fake::default()
    };
    let mut picker = Fake {
        choices: [Some("prod".into()), Some("true".into())].into(),
        paths: [Some("manifest.yml".into()), Some("deploy".into())].into(),
        ..Fake::default()
    };
    let collected = collect(
        &recipe,
        &config,
        Path::new("/project"),
        &mut prompts,
        &mut picker,
    )
    .unwrap();
    assert_eq!(
        collected.value("text"),
        Some(&ParameterValue::Scalar("literal".into()))
    );
    assert_eq!(
        collected.value("environment"),
        Some(&ParameterValue::Scalar("prod".into()))
    );
    assert_eq!(
        collected.value("confirm"),
        Some(&ParameterValue::Scalar("true".into()))
    );
    assert_eq!(
        collected.value("manifest"),
        Some(&ParameterValue::Scalar("manifest.yml".into()))
    );
    assert_eq!(
        collected.value("directory"),
        Some(&ParameterValue::Scalar("deploy".into()))
    );
}

#[test]
fn explicit_prompt_cancellation_returns_cancelled() {
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![param("value", None, ParameterKind::Singular, false)],
        ..Recipe::default()
    };
    let mut prompts = Fake {
        inputs: [None].into(),
        ..Fake::default()
    };
    let error = collect(
        &recipe,
        &Config::default(),
        Path::new("."),
        &mut prompts,
        &mut Fake::default(),
    )
    .unwrap_err();
    assert!(matches!(error, jtv::Error::Cancelled));
}
