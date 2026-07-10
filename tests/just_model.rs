use jtv::{just::parse_project, model::ParameterKind};

const DUMP: &[u8] = include_bytes!("fixtures/json/just-1.53.json");

#[test]
fn parses_and_recursively_flattens_public_recipes_and_aliases() {
    let project = parse_project(DUMP, None).unwrap();
    let names: Vec<_> = project
        .recipes
        .iter()
        .map(|recipe| recipe.namepath.as_str())
        .collect();
    assert_eq!(names, ["b", "build", "ops::cloud::status", "ops::deploy"]);
    assert_eq!(
        project.recipe("b").unwrap().alias_target.as_deref(),
        Some("build")
    );
    assert!(project.recipe("private").is_none());
    assert_eq!(project.warnings, ["fixture warning"]);
}

#[test]
fn preserves_recipe_and_parameter_semantics() {
    let project = parse_project(DUMP, None).unwrap();
    let recipe = project.recipe("build").unwrap();
    assert_eq!(recipe.doc.as_deref(), Some("Builds the app"));
    assert_eq!(recipe.group.as_deref(), Some("dev"));
    assert_eq!(recipe.dependencies, ["prepare"]);
    assert!(recipe.quiet);
    assert_eq!(recipe.body, ["cargo build"]);
    assert_eq!(recipe.parameters[0].default.as_deref(), Some("debug"));
    assert_eq!(recipe.parameters[1].kind, ParameterKind::Star);
    assert!(recipe.parameters[1].flag);
    assert_eq!(recipe.parameters[1].long.as_deref(), Some("features"));
    assert_eq!(recipe.parameters[1].short.as_deref(), Some("f"));
    assert_eq!(recipe.parameters[1].help.as_deref(), Some("Cargo features"));
    assert_eq!(recipe.parameters[1].pattern.as_deref(), Some("[\"tuple\"]"));
}

#[test]
fn filters_recursively_by_exact_module_namepath() {
    let project = parse_project(DUMP, Some("ops")).unwrap();
    let names: Vec<_> = project
        .recipes
        .iter()
        .map(|recipe| recipe.namepath.as_str())
        .collect();
    assert_eq!(names, ["ops::cloud::status", "ops::deploy"]);
    assert!(parse_project(DUMP, Some("op")).unwrap().recipes.is_empty());
}

#[test]
fn additive_unknown_fields_are_ignored() {
    parse_project(DUMP, None).unwrap();
}
