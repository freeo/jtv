use jtv::{
    config::{Config, ParameterConfig},
    model::{Parameter, ParameterKind, Project, Recipe},
};

fn project() -> Project {
    Project {
        recipes: vec![Recipe {
            name: "deploy".into(),
            namepath: "ops::deploy".into(),
            parameters: vec![Parameter {
                name: "target".into(),
                default: None,
                default_expression: None,
                kind: ParameterKind::Singular,
                flag: false,
                long: None,
                short: None,
                help: None,
                value: None,
                pattern: None,
            }],
            ..Recipe::default()
        }],
        warnings: vec![],
    }
}

#[test]
fn searches_upward_and_parses_all_metadata_kinds() {
    let temp = tempfile::tempdir().unwrap();
    let nested = temp.path().join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(
        temp.path().join(".jtv.toml"),
        r#"
[recipes."ops::deploy".params.target]
type = "choice"
values = ["dev", "prod"]
"#,
    )
    .unwrap();
    let loaded = Config::load_upward(&nested).unwrap();
    assert_eq!(
        loaded.config.parameter("ops::deploy", "target"),
        Some(&ParameterConfig::Choice {
            values: vec!["dev".into(), "prod".into()]
        })
    );
    loaded
        .config
        .validate(&project(), loaded.path.as_deref().unwrap())
        .unwrap();
}

#[test]
fn absent_config_is_empty_and_typos_are_errors() {
    let temp = tempfile::tempdir().unwrap();
    assert_eq!(
        Config::load_upward(temp.path()).unwrap().config,
        Config::default()
    );
    let config: Config = toml::from_str("[recipes.missing.params.x]\ntype='string'").unwrap();
    assert!(
        config
            .validate(&project(), temp.path())
            .unwrap_err()
            .to_string()
            .contains("unknown recipe")
    );
}

#[test]
fn unknown_parameter_and_unknown_fields_are_rejected() {
    let config: Config =
        toml::from_str("[recipes.\"ops::deploy\".params.typo]\ntype='secret'").unwrap();
    assert!(
        config
            .validate(&project(), std::path::Path::new(".jtv.toml"))
            .unwrap_err()
            .to_string()
            .contains("unknown parameter")
    );
    assert!(toml::from_str::<Config>("surprise=true").is_err());
}
