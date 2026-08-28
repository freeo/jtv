use jtv::{
    command::build_plan,
    invocation::Invocation,
    model::{Parameter, ParameterKind, Recipe},
    parameters::{CollectedParameters, ParameterValue},
};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

fn parameter(name: &str, default: Option<&str>, flag: bool, long: Option<&str>) -> Parameter {
    Parameter {
        name: name.into(),
        default: default.map(Into::into),
        default_expression: None,
        kind: ParameterKind::Singular,
        flag,
        long: long.map(Into::into),
        short: None,
        help: None,
        value: None,
        pattern: None,
    }
}
fn collected(
    values: impl IntoIterator<Item = (&'static str, ParameterValue)>,
) -> CollectedParameters {
    // Construct through serde-independent public collection is deliberately unavailable;
    // use the normal collector in integration tests. This local helper relies on a constructor.
    jtv::parameters::CollectedParameters::new(
        values
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        vec![],
    )
}

#[test]
fn preserves_adversarial_values_as_single_os_arguments() {
    let recipe = Recipe {
        name: "attack".into(),
        namepath: "attack".into(),
        parameters: vec![parameter("value", None, false, None)],
        ..Recipe::default()
    };
    for value in [
        "a b",
        "'single'",
        "\"double\"",
        "; touch NO",
        "$(touch NO)",
        "a|b",
        ">file",
        "--leading",
        "λ",
        "line1\nline2",
    ] {
        let plan = build_plan(
            &Invocation::new(
                PathBuf::from("/tmp"),
                Some(PathBuf::from("odd name.just")),
                None,
                true,
            ),
            &recipe,
            &collected([("value", ParameterValue::Scalar(value.into()))]),
        )
        .unwrap();
        assert_eq!(plan.args.last(), Some(&OsString::from(value)));
        assert_eq!(plan.args.iter().filter(|arg| *arg == value).count(), 1);
    }
}

#[test]
fn materializes_preceding_default_only_for_later_value_and_emits_flags() {
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![
            parameter("first", Some("one"), false, None),
            parameter("second", Some("two"), false, None),
            parameter("verbose", None, true, Some("verbose")),
        ],
        ..Recipe::default()
    };
    let plan = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &collected([
            ("first", ParameterValue::Scalar("one".into())),
            ("second", ParameterValue::Scalar("changed".into())),
            ("verbose", ParameterValue::Flag(true)),
        ]),
    )
    .unwrap();
    assert_eq!(plan.args, vec!["r", "--verbose", "one", "changed"]);
    let defaults = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &collected([
            ("first", ParameterValue::Scalar("one".into())),
            ("second", ParameterValue::Scalar("two".into())),
            ("verbose", ParameterValue::Flag(false)),
        ]),
    )
    .unwrap();
    assert_eq!(defaults.args, vec!["r"]);
}

#[test]
fn emits_value_taking_variadic_flag_as_flag_then_literal_values() {
    let mut flag = parameter("features", None, true, Some("features"));
    flag.kind = ParameterKind::Star;
    flag.value = Some("value".into());
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![flag],
        ..Recipe::default()
    };
    let plan = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &collected([(
            "features",
            ParameterValue::Many(vec!["a b".into(), ";bad".into()]),
        )]),
    )
    .unwrap();
    assert_eq!(plan.args, vec!["r", "--features", "a b", ";bad"]);
    let omitted = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &collected([("features", ParameterValue::Many(vec![]))]),
    )
    .unwrap();
    assert_eq!(omitted.args, vec!["r"]);
}

#[test]
fn secret_is_only_redacted_in_display_not_executable_argv() {
    let values = CollectedParameters::new(
        BTreeMap::from([("token".into(), ParameterValue::Scalar("top secret".into()))]),
        vec!["token".into()],
    );
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![parameter("token", None, false, None)],
        ..Recipe::default()
    };
    let plan = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &values,
    )
    .unwrap();
    assert_eq!(plan.args.last().unwrap(), "top secret");
    assert!(!plan.display_redacted().contains("top secret"));
    assert!(plan.display_redacted().contains("REDACTED"));
    assert!(plan.contains_secret);
    assert_eq!(plan.history_command_zsh().unwrap(), None);
}

#[test]
fn only_an_emitted_configured_secret_suppresses_history() {
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![
            parameter("token", Some("configured-default"), false, None),
            parameter("later", Some("tail"), false, None),
        ],
        ..Recipe::default()
    };
    let omitted_secret = CollectedParameters::new(
        BTreeMap::from([
            (
                "token".into(),
                ParameterValue::Scalar("configured-default".into()),
            ),
            ("later".into(), ParameterValue::Scalar("tail".into())),
        ]),
        vec!["token".into()],
    );
    let plan = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &omitted_secret,
    )
    .unwrap();
    assert_eq!(plan.args, vec!["r"]);
    assert!(!plan.contains_secret);
    assert!(plan.history_command_zsh().unwrap().is_some());

    let materialized_secret = CollectedParameters::new(
        BTreeMap::from([
            (
                "token".into(),
                ParameterValue::Scalar("configured-default".into()),
            ),
            ("later".into(), ParameterValue::Scalar("changed".into())),
        ]),
        vec!["token".into()],
    );
    let plan = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &materialized_secret,
    )
    .unwrap();
    assert_eq!(plan.args, vec!["r", "configured-default", "changed"]);
    assert!(plan.contains_secret);
    assert_eq!(plan.history_command_zsh().unwrap(), None);
}

#[test]
fn emitted_secret_flag_suppresses_history_even_without_a_value_argument() {
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![parameter("private_mode", None, true, None)],
        ..Recipe::default()
    };
    let values = CollectedParameters::new(
        BTreeMap::from([("private_mode".into(), ParameterValue::Flag(true))]),
        vec!["private_mode".into()],
    );
    let plan = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &values,
    )
    .unwrap();
    assert_eq!(plan.args, vec!["r", "--private-mode"]);
    assert!(plan.contains_secret);
    assert_eq!(plan.history_command_zsh().unwrap(), None);
}

#[cfg(unix)]
#[test]
fn zsh_history_command_round_trips_adversarial_and_non_utf8_argv() {
    use std::{os::unix::ffi::OsStringExt, process::Command};

    let args = vec![
        "%s\\0".into(),
        "".into(),
        "a b".into(),
        "'single'".into(),
        "$(touch NO)".into(),
        "line1\nline2".into(),
        "λ".into(),
        OsString::from_vec(vec![0xff, b'f']),
    ];
    let plan = jtv::command::CommandPlan {
        program: "printf".into(),
        cwd: ".".into(),
        args: args.clone(),
        redacted_args: vec![],
        contains_secret: false,
    };
    let command = plan.history_command_zsh().unwrap().unwrap();
    let output = Command::new("zsh")
        .args(["-f", "-c", &command])
        .output()
        .expect("zsh is required for this test");
    assert!(output.status.success());
    let expected = args
        .iter()
        .skip(1)
        .flat_map(|arg| {
            let mut bytes = arg.clone().into_vec();
            bytes.push(0);
            bytes
        })
        .collect::<Vec<_>>();
    assert_eq!(output.stdout, expected);
}

#[cfg(unix)]
#[test]
fn zsh_history_command_rejects_nul_bytes() {
    use std::os::unix::ffi::OsStringExt;

    let plan = jtv::command::CommandPlan {
        program: "just".into(),
        cwd: ".".into(),
        args: vec![OsString::from_vec(vec![b'a', 0, b'b'])],
        redacted_args: vec![],
        contains_secret: false,
    };
    assert!(
        plan.history_command_zsh()
            .unwrap_err()
            .to_string()
            .contains("NUL")
    );
}

#[test]
fn opaque_expression_defaults_are_omitted_or_require_explicit_materialization() {
    let mut expression = parameter("first", None, false, None);
    expression.default_expression = Some(r#"["concatenate","x"," y"]"#.into());
    let recipe = Recipe {
        name: "r".into(),
        namepath: "r".into(),
        parameters: vec![expression, parameter("second", Some("tail"), false, None)],
        ..Recipe::default()
    };
    let omitted = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &collected([
            ("first", ParameterValue::Omitted),
            ("second", ParameterValue::Scalar("tail".into())),
        ]),
    )
    .unwrap();
    assert_eq!(omitted.args, vec!["r"]);

    let error = build_plan(
        &Invocation::new(".".into(), None, None, false),
        &recipe,
        &collected([
            ("first", ParameterValue::Omitted),
            ("second", ParameterValue::Scalar("custom".into())),
        ]),
    )
    .unwrap_err();
    assert!(error.to_string().contains("non-literal just default"));
}
