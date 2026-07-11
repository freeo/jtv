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
    labels: Vec<String>,
    completion_calls: usize,
}
impl Prompter for Fake {
    fn input(&mut self, label: &str, _: Option<&str>, _: bool) -> Result<Option<String>> {
        self.labels.push(label.into());
        Ok(self.inputs.pop_front().unwrap())
    }
    fn variadic(&mut self, label: &str, _: usize, _: bool) -> Result<Option<Vec<String>>> {
        self.labels.push(label.into());
        Ok(self.many.pop_front().unwrap())
    }
    fn input_with_completion(
        &mut self,
        label: &str,
        default: Option<&str>,
        _: &Path,
        _: &mut dyn Picker,
    ) -> Result<Option<String>> {
        self.completion_calls += 1;
        self.input(label, default, false)
    }
    fn variadic_with_completion(
        &mut self,
        label: &str,
        minimum: usize,
        _: &Path,
        _: &mut dyn Picker,
    ) -> Result<Option<Vec<String>>> {
        self.completion_calls += 1;
        self.variadic(label, minimum, false)
    }
}
impl Picker for Fake {
    fn choose(&mut self, label: &str, _: &[String]) -> Result<Option<String>> {
        self.labels.push(label.into());
        Ok(self.choices.pop_front().unwrap())
    }
    fn choose_path(&mut self, label: &str, _: &Path, _: PathKind) -> Result<Option<PathBuf>> {
        self.labels.push(label.into());
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
    let mut verbose = param("verbose", None, ParameterKind::Singular, true);
    verbose.value = Some("true".into());
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![
            param("target", Some("dev"), ParameterKind::Singular, false),
            param("token", None, ParameterKind::Singular, false),
            verbose,
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
    assert_eq!(
        prompts.completion_calls, 3,
        "both ordinary inputs and later-positional materialization support TAB"
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
    assert_eq!(
        picker.labels,
        [
            "[2/5] Choose environment — choice",
            "[3/5] Choose confirm — boolean",
            "[4/5] Select manifest — file",
            "[5/5] Select directory — directory",
        ]
    );
}

#[test]
fn fixed_value_flag_uses_boolean_picker_but_value_option_prompts() {
    let mut fixed = param("force", Some("disabled"), ParameterKind::Singular, true);
    fixed.long = Some("force".into());
    fixed.value = Some("enabled".into());
    let mut option = param("target", Some("default"), ParameterKind::Singular, true);
    option.short = Some("t".into());
    let recipe = Recipe {
        name: "flags".into(),
        namepath: "flags".into(),
        parameters: vec![fixed, option],
        ..Recipe::default()
    };
    let mut prompts = Fake {
        inputs: [Some("production".into())].into(),
        ..Fake::default()
    };
    let mut picker = Fake {
        choices: [Some("true".into())].into(),
        ..Fake::default()
    };
    let collected = collect(
        &recipe,
        &Config::default(),
        Path::new("."),
        &mut prompts,
        &mut picker,
    )
    .unwrap();
    assert_eq!(collected.value("force"), Some(&ParameterValue::Flag(true)));
    assert_eq!(
        collected.value("target"),
        Some(&ParameterValue::Scalar("production".into()))
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

#[test]
fn prompt_labels_explain_progress_type_requirements_and_hide_secret_defaults() {
    let mut token = param(
        "token",
        Some("hidden-default"),
        ParameterKind::Singular,
        false,
    );
    token.help = Some("API credential".into());
    let recipe = Recipe {
        name: "deploy".into(),
        namepath: "deploy".into(),
        parameters: vec![token, param("path", None, ParameterKind::Singular, false)],
        ..Recipe::default()
    };
    let config: Config = toml::from_str(
        "[recipes.deploy.params.token]\ntype='secret'\n[recipes.deploy.params.path]\ntype='file'",
    )
    .unwrap();
    let mut prompts = Fake {
        inputs: [Some("provided".into())].into(),
        ..Fake::default()
    };
    let mut picker = Fake {
        paths: [Some("artifact.txt".into())].into(),
        ..Fake::default()
    };
    collect(&recipe, &config, Path::new("."), &mut prompts, &mut picker).unwrap();
    assert_eq!(
        prompts.labels,
        ["[1/2] token (secret, default: [REDACTED]) — API credential"]
    );
    assert_eq!(picker.labels, ["[2/2] Select path — file"]);
    assert!(!format!("{:?}", prompts.labels).contains("hidden-default"));
    assert_eq!(
        prompts.completion_calls, 0,
        "secrets never enable TAB paths"
    );
}

#[test]
fn ordinary_singular_and_variadic_strings_enable_tab_path_completion() {
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![
            param("single", None, ParameterKind::Singular, false),
            param("many", None, ParameterKind::Star, false),
        ],
        ..Recipe::default()
    };
    let mut prompts = Fake {
        inputs: [Some("docs/read me.md".into())].into(),
        many: [Some(vec!["src".into()])].into(),
        ..Fake::default()
    };
    collect(
        &recipe,
        &Config::default(),
        Path::new("/project"),
        &mut prompts,
        &mut Fake::default(),
    )
    .unwrap();
    assert_eq!(prompts.completion_calls, 2);
}
