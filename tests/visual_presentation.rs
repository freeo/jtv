use std::path::PathBuf;

use jtv::{
    config::Config,
    invocation::Invocation,
    model::{Parameter, ParameterKind, Project, Recipe},
    presentation::{PresentationOptions, ResolvedColorMode, ResolvedIconMode},
    session::{CatalogTarget, SessionState, WorkspaceCatalog},
    television,
    workspace::WorkspaceOrigin,
};

fn parameter(name: &str, default: Option<&str>, kind: ParameterKind) -> Parameter {
    Parameter {
        name: name.into(),
        default: default.map(str::to_owned),
        kind,
        ..Parameter::default()
    }
}

fn workspace_state(icons: ResolvedIconMode) -> SessionState {
    let root = CatalogTarget {
        origin: WorkspaceOrigin::Root,
        invocation: Invocation::new(
            PathBuf::from("/tmp/project"),
            Some(PathBuf::from("/tmp/project/justfile")),
            None,
            false,
        ),
        project: Project {
            recipes: vec![
                Recipe {
                    name: "build".into(),
                    namepath: "build".into(),
                    ..Recipe::default()
                },
                Recipe {
                    name: "publish".into(),
                    namepath: "docker::publish".into(),
                    module: Some("docker".into()),
                    ..Recipe::default()
                },
            ],
            warnings: vec![],
        },
        config: Config::default(),
    };
    let child = CatalogTarget {
        origin: WorkspaceOrigin::Subfolder {
            relative_justfile: PathBuf::from("supabase/justfile"),
            label: "supabase/".into(),
        },
        invocation: Invocation::new(
            PathBuf::from("/tmp/project"),
            Some(PathBuf::from("/tmp/project/supabase/justfile")),
            None,
            false,
        ),
        project: Project {
            recipes: vec![Recipe {
                name: "migrate".into(),
                namepath: "migrate".into(),
                doc: Some("Update database schema".into()),
                ..Recipe::default()
            }],
            warnings: vec![],
        },
        config: Config::default(),
    };
    SessionState::new_with_catalog(
        WorkspaceCatalog::new(PathBuf::from("/tmp/project"), vec![root, child], vec![]),
        PresentationOptions {
            color: ResolvedColorMode::Plain,
            source_color: ResolvedColorMode::Plain,
            icons,
            compact: false,
        },
    )
    .unwrap()
}

#[test]
fn source_views_partition_catalog_and_subfolders_keep_path_identity() {
    let state = workspace_state(ResolvedIconMode::Unicode);
    let root = television::source_output(&state, television::SourceView::Root).unwrap();
    let subfolders = television::source_output(&state, television::SourceView::Subfolders).unwrap();
    let modules = television::source_output(&state, television::SourceView::Modules).unwrap();
    let all = television::source_output(&state, television::SourceView::All).unwrap();

    assert!(root.contains("🔷 build"));
    assert!(!root.contains("publish"));
    assert!(!root.contains("migrate"));
    assert!(modules.contains("🐳 docker::publish"));
    assert!(!modules.contains("build"));
    assert_eq!(subfolders.lines().count(), 1);
    assert!(subfolders.contains("📁 supabase/  migrate"));
    assert!(
        subfolders
            .split('\t')
            .nth(2)
            .unwrap()
            .contains("supabase/justfile")
    );
    assert_eq!(all.lines().count(), 3);

    let child_id = state
        .catalog
        .selections
        .iter()
        .find_map(|(id, selection)| (selection.target_index == 1).then_some(id))
        .unwrap();
    let preview = television::preview(&state, child_id).unwrap();
    assert!(preview.contains("Source: supabase/"));
    assert!(preview.contains("Justfile: supabase/justfile"));
}

#[test]
fn subfolder_origin_has_ascii_and_no_icon_fallbacks() {
    let ascii = workspace_state(ResolvedIconMode::Ascii);
    let row = television::source_output(&ascii, television::SourceView::Subfolders).unwrap();
    assert!(row.contains("[dir] supabase/  migrate"));

    let none = workspace_state(ResolvedIconMode::None);
    let row = television::source_output(&none, television::SourceView::Subfolders).unwrap();
    assert!(row.contains("\tsupabase/  migrate\t"));
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
    let rows = television::source_output(&state, television::SourceView::All).unwrap();
    let lines = rows.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("🔷"));
    assert!(lines[0].contains("\x1b[0;36mbuild\x1b[0m"));
    assert!(lines[0].contains("\x1b[0;91mtarget\x1b[0m"));
    assert!(!lines[0].split('\t').nth(1).unwrap().contains("<required>"));
    assert!(!lines[0].contains("\x1b[0;95mbuild"));
    assert!(lines[0].contains("\x1b[0;95mprepare\x1b[0m"));
    assert!(!lines[0].split('\t').nth(1).unwrap().contains('→'));
    assert!(
        !lines[0]
            .split('\t')
            .nth(1)
            .unwrap()
            .contains("Build the application")
    );
    assert!(lines[1].contains("🐳"));
    assert!(lines[1].contains("\x1b[0;94mdocker\x1b[0m::"));
    assert!(lines[1].contains("\x1b[0;36mpublish\x1b[0m"));
    assert!(!lines[1].split('\t').nth(1).unwrap().contains("release"));
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
    let rows = television::source_output(&state, television::SourceView::All).unwrap();
    assert!(rows.contains("[core] build  target:<required>"));
    assert!(rows.contains("[docker] docker::publish  token:[REDACTED] --features:<required>*"));
    assert!(!rows.contains("→ prepare"));
    assert!(!rows.contains('\x1b'));
}

#[test]
fn details_preview_is_structured_typed_and_secret_safe() {
    let state = state(ResolvedColorMode::Color, ResolvedIconMode::Unicode, false);
    let id = state
        .catalog
        .selections
        .iter()
        .find_map(|(id, selection)| (selection.recipe_namepath == "docker::publish").then_some(id))
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
