use std::{collections::BTreeMap, path::PathBuf};

use jtv::{
    config::{Config, ParameterConfig, RecipeConfig},
    invocation::Invocation,
    model::{Parameter, Project, Recipe},
    presentation::PresentationOptions,
    session::{CatalogTarget, SessionState, WorkspaceCatalog, load, validate_id},
    workspace::WorkspaceOrigin,
};

fn project(namepath: &str) -> Project {
    Project {
        recipes: vec![Recipe {
            name: namepath.rsplit("::").next().unwrap().into(),
            namepath: namepath.into(),
            parameters: vec![Parameter {
                name: "token".into(),
                ..Parameter::default()
            }],
            ..Recipe::default()
        }],
        warnings: vec![],
    }
}

fn invocation(root: &str, justfile: &str) -> Invocation {
    Invocation::new(
        PathBuf::from(root),
        Some(PathBuf::from(justfile)),
        None,
        false,
    )
}

fn state() -> SessionState {
    let mut root_config = Config::default();
    root_config.recipes.insert(
        "test".into(),
        RecipeConfig {
            params: BTreeMap::from([("token".into(), ParameterConfig::Secret)]),
        },
    );
    let targets = vec![
        CatalogTarget {
            origin: WorkspaceOrigin::Root,
            invocation: invocation("/workspace", "/workspace/justfile"),
            project: project("test"),
            config: root_config,
        },
        CatalogTarget {
            origin: WorkspaceOrigin::Subfolder {
                relative_justfile: "supabase/justfile".into(),
                label: "supabase/".into(),
            },
            invocation: invocation("/workspace", "/workspace/supabase/justfile"),
            project: project("test"),
            config: Config::default(),
        },
    ];
    SessionState::new_with_catalog(
        WorkspaceCatalog::new(PathBuf::from("/workspace"), targets, vec![]),
        PresentationOptions::default(),
    )
    .unwrap()
}

#[test]
fn duplicate_namepaths_resolve_to_distinct_owning_targets_and_configs() {
    let state = state();
    let ids = state.catalog.selections.keys().collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    ids.iter().for_each(|id| validate_id(id).unwrap());

    let root = state.resolve(ids[0]).unwrap();
    let child = state.resolve(ids[1]).unwrap();
    assert_eq!(root.recipe.namepath, "test");
    assert_eq!(child.recipe.namepath, "test");
    assert!(matches!(root.target.origin, WorkspaceOrigin::Root));
    assert!(matches!(
        child.target.origin,
        WorkspaceOrigin::Subfolder { .. }
    ));
    assert_eq!(
        root.target.config.parameter("test", "token"),
        Some(&ParameterConfig::Secret)
    );
    assert_eq!(child.target.config.parameter("test", "token"), None);
}

#[test]
fn serialized_session_rejects_protocol_target_recipe_and_origin_tampering() {
    let state = state();
    let original = serde_json::to_value(&state).unwrap();
    for mutate in [
        |value: &mut serde_json::Value| value["protocol"] = 1.into(),
        |value: &mut serde_json::Value| {
            let id = value["catalog"]["selections"]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .clone();
            value["catalog"]["selections"][id]["target_index"] = 99.into();
        },
        |value: &mut serde_json::Value| {
            let id = value["catalog"]["selections"]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .clone();
            value["catalog"]["selections"][id]["recipe_namepath"] = "missing".into();
        },
        |value: &mut serde_json::Value| {
            value["catalog"]["targets"][1]["origin"]["Subfolder"]["relative_justfile"] =
                "../escape.just".into();
        },
    ] {
        let mut value = original.clone();
        mutate(&mut value);
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(load(temp.path()).is_err(), "accepted {value}");
    }
}

#[test]
fn selection_ids_are_bound_to_the_launch_session() {
    let first = state();
    let mut second = state();
    let foreign_id = first.catalog.selections.keys().next().unwrap().clone();
    let local_id = second.catalog.selections.keys().next().unwrap().clone();
    let selection = second.catalog.selections.remove(&local_id).unwrap();
    second.catalog.selections.insert(foreign_id, selection);

    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), serde_json::to_vec(&second).unwrap()).unwrap();
    assert!(load(temp.path()).is_err());
}

#[test]
fn missing_and_duplicate_selection_references_are_rejected() {
    let mut missing = state();
    let id = missing.catalog.selections.keys().next().unwrap().clone();
    missing.catalog.selections.remove(&id);
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), serde_json::to_vec(&missing).unwrap()).unwrap();
    assert!(load(temp.path()).is_err());

    let mut duplicate = state();
    let ids = duplicate
        .catalog
        .selections
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let first = duplicate.catalog.selections[&ids[0]].clone();
    duplicate.catalog.selections.insert(ids[1].clone(), first);
    std::fs::write(temp.path(), serde_json::to_vec(&duplicate).unwrap()).unwrap();
    assert!(load(temp.path()).is_err());
}
