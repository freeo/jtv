use std::path::PathBuf;

use jtv::{
    config::Config,
    invocation::Invocation,
    just::{load_project, render_preview},
    presentation::{PresentationOptions, ResolvedColorMode, ResolvedIconMode},
    session::SessionState,
    television,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/justfiles")
}

#[test]
fn loads_real_just_dump_with_modules_and_aliases() {
    let invocation = Invocation::new(fixture_dir(), None, None, false)
        .canonicalized()
        .unwrap();
    let project = load_project(&invocation).unwrap();
    assert!(project.recipe("build").is_some());
    assert_eq!(
        project.recipe("b").unwrap().alias_target.as_deref(),
        Some("build")
    );
    assert!(project.recipe("ops::deploy").is_some());
    assert!(project.recipe("ops::hidden").is_none());
}

#[test]
fn explicit_justfile_and_module_filter_are_honored() {
    let invocation = Invocation::new(
        fixture_dir(),
        Some(PathBuf::from("justfile")),
        Some("ops".into()),
        false,
    )
    .canonicalized()
    .unwrap();
    let project = load_project(&invocation).unwrap();
    assert_eq!(project.recipes.len(), 1);
    assert_eq!(project.recipes[0].namepath, "ops::deploy");
    let preview = render_preview(&invocation, &project.recipes[0]).unwrap();
    assert!(preview.contains("deploy environment:"));
}

#[test]
fn preview_uses_real_just_show() {
    let invocation = Invocation::new(fixture_dir(), None, None, false)
        .canonicalized()
        .unwrap();
    let project = load_project(&invocation).unwrap();
    let preview = render_preview(&invocation, project.recipe("build").unwrap()).unwrap();
    assert!(preview.contains("build target='debug': prepare"));
}

#[test]
fn definition_preview_styles_real_just_show_without_affecting_identity() {
    let invocation = Invocation::new(fixture_dir(), None, None, false)
        .canonicalized()
        .unwrap();
    let project = load_project(&invocation).unwrap();
    let state = SessionState::new_with_presentation(
        invocation,
        project,
        Config::default(),
        PresentationOptions {
            color: ResolvedColorMode::Color,
            source_color: ResolvedColorMode::Color,
            icons: ResolvedIconMode::Ascii,
            compact: false,
        },
    )
    .unwrap();
    let id = state
        .catalog
        .selections
        .iter()
        .find_map(|(id, selection)| (selection.recipe_namepath == "build").then_some(id))
        .unwrap();
    let preview = television::definition_preview(&state, id).unwrap();
    assert!(preview.contains("build target='debug': prepare"));
    assert!(preview.contains("\x1b[0;36m"));
    assert!(television::definition_preview(&state, "build").is_err());
}

#[test]
fn preserves_expression_defaults_as_opaque_and_resolves_alias_parameters() {
    let root = tempfile::tempdir().unwrap();
    let justfile = root.path().join("justfile");
    std::fs::write(
        &justfile,
        r#"
value := "x"

target first=(value + " y") second="tail":
    @printf '%s %s\n' "{{first}}" "{{second}}"

alias shortcut := target
"#,
    )
    .unwrap();
    let invocation = Invocation::new(root.path().into(), Some(justfile), None, false)
        .canonicalized()
        .unwrap();
    let project = load_project(&invocation).unwrap();
    let target = project.recipe("target").unwrap();
    assert!(target.parameters[0].default.is_none());
    assert!(target.parameters[0].default_expression.is_some());
    assert_eq!(target.parameters[1].default.as_deref(), Some("tail"));
    let alias = project.recipe("shortcut").unwrap();
    assert_eq!(alias.alias_target.as_deref(), Some("target"));
    assert_eq!(alias.parameters, target.parameters);
}

#[test]
fn recognizes_real_just_options_and_fixed_value_flags() {
    let temp = tempfile::tempdir().unwrap();
    let justfile = temp.path().join("justfile");
    std::fs::write(
        &justfile,
        r#"
[arg("force", long="force", value="enabled")]
[arg("target", short="t")]
flags force="disabled" target="default":
    @echo {{force}} {{target}}
"#,
    )
    .unwrap();
    let invocation = Invocation::new(temp.path().into(), Some(justfile), None, false);
    let project = load_project(&invocation).unwrap();
    let recipe = project.recipe("flags").unwrap();
    assert!(recipe.parameters[0].flag);
    assert_eq!(recipe.parameters[0].long.as_deref(), Some("force"));
    assert_eq!(recipe.parameters[0].value.as_deref(), Some("enabled"));
    assert!(recipe.parameters[1].flag);
    assert_eq!(recipe.parameters[1].short.as_deref(), Some("t"));
    assert_eq!(recipe.parameters[1].value, None);
}
