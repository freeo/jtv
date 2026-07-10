use std::path::PathBuf;

use jtv::{
    config::Config,
    invocation::Invocation,
    model::{Parameter, ParameterKind, Project, Recipe},
    presentation::{PresentationOptions, ResolvedColorMode, ResolvedIconMode},
    session::SessionState,
    television,
};

fn parameter(name: &str, default: Option<&str>, kind: ParameterKind) -> Parameter {
    Parameter {
        name: name.into(),
        default: default.map(str::to_owned),
        kind,
        ..Parameter::default()
    }
}

fn state(color: ResolvedColorMode, icons: ResolvedIconMode, compact: bool) -> SessionState {
    let core = Recipe {
        name: "build".into(),
        namepath: "build".into(),
        doc: Some("Build the application".into()),
        parameters: vec![parameter("target", None, ParameterKind::Singular)],
        dependencies: vec!["prepare".into()],
        body: vec!["cargo build".into()],
        ..Recipe::default()
    };
    let mut secret = parameter("token", Some("must-not-appear"), ParameterKind::Singular);
    secret.help = Some("Registry credential".into());
    let mut features = parameter("features", None, ParameterKind::Star);
    features.flag = true;
    features.long = Some("features".into());
    features.help = Some("Cargo features".into());
    let docker = Recipe {
        name: "publish".into(),
        namepath: "docker::publish".into(),
        module: Some("docker".into()),
        doc: Some("Publish safely\x1b]52;c;bad\x07".into()),
        group: Some("release".into()),
        dependencies: vec!["build".into()],
        parameters: vec![secret, features],
        attributes: vec!["group:release".into()],
        quiet: true,
        shebang: true,
        alias_target: Some("docker::push".into()),
        body: vec!["#!/usr/bin/env bash".into(), "echo publish".into()],
        ..Recipe::default()
    };
    let config: Config =
        toml::from_str("[recipes.'docker::publish'.params.token]\ntype = 'secret'\n").unwrap();
    SessionState::new_with_presentation(
        Invocation::new(PathBuf::from("/tmp/project"), None, None, false),
        Project {
            recipes: vec![docker, core],
            warnings: vec![],
        },
        config,
        PresentationOptions {
            color,
            source_color: color,
            icons,
            compact,
        },
    )
    .unwrap()
}

#[test]
fn styled_rows_restore_legacy_roles_and_core_first_order() {
    let state = state(ResolvedColorMode::Color, ResolvedIconMode::Unicode, false);
    let rows = television::source_output(&state).unwrap();
    let lines = rows.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("🔷"));
    assert!(lines[0].contains("\x1b[0;36mbuild\x1b[0m"));
    assert!(lines[0].contains("\x1b[0;91mtarget\x1b[0m"));
    assert!(!lines[0].contains("\x1b[0;95mbuild"));
    assert!(lines[0].contains("\x1b[0;95mprepare\x1b[0m"));
    assert!(lines[0].contains("Build the application"));
    assert!(lines[1].contains("🐳"));
    assert!(lines[1].contains("\x1b[0;94mdocker\x1b[0m::"));
    assert!(lines[1].contains("\x1b[0;36mpublish\x1b[0m"));
    assert!(lines[1].contains("#\x1b[0m\x1b[2mrelease"));
    assert!(lines[1].contains("[REDACTED]"));
    assert!(!rows.contains("must-not-appear"));
    assert!(!rows.contains("\x1b]52"));
    for line in lines {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 3);
        assert!(fields[0].starts_with("jtv-"));
        assert!(!fields[0].contains('\x1b'));
        assert!(!fields[2].contains('\x1b'));
    }
}

#[test]
fn plain_ascii_and_compact_modes_preserve_meaning_without_controls() {
    let state = state(ResolvedColorMode::Plain, ResolvedIconMode::Ascii, true);
    let rows = television::source_output(&state).unwrap();
    assert!(rows.contains("[core] build target:<required>"));
    assert!(rows.contains("[docker] docker::publish token:[REDACTED] --features:<required>*"));
    assert!(!rows.contains("→ prepare"));
    assert!(!rows.contains('\x1b'));
}

#[test]
fn details_preview_is_structured_typed_and_secret_safe() {
    let state = state(ResolvedColorMode::Color, ResolvedIconMode::Unicode, false);
    let id = state
        .selections
        .iter()
        .find_map(|(id, recipe)| (recipe == "docker::publish").then_some(id))
        .unwrap();
    let preview = television::preview(&state, id).unwrap();
    for expected in [
        "Module: ",
        "docker::publish",
        "alias → docker::push",
        "group: release",
        "Summary",
        "Parameters",
        "Dependencies",
        "Attributes",
        "Recipe",
        "Registry credential",
        "zero or more",
        "#!/usr/bin/env bash",
    ] {
        assert!(
            preview.contains(expected),
            "missing {expected:?}: {preview:?}"
        );
    }
    assert!(preview.contains("\x1b[1;37mModule: \x1b[0m"));
    assert!(preview.contains("\x1b[0;95m→ \x1b[0m"));
    assert!(preview.contains("[REDACTED]"));
    assert!(!preview.contains("must-not-appear"));
    assert!(!preview.contains("\x1b]52"));
}
