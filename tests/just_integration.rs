use std::path::PathBuf;

use jtv::{
    invocation::Invocation,
    just::{load_project, render_preview},
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
