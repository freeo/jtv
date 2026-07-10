use std::path::PathBuf;

use jtv::{
    invocation::Invocation,
    model::{Project, Recipe},
    session::{SessionFile, SessionState, load, validate_id},
    television,
};

fn state() -> SessionState {
    let recipe = Recipe {
        name: "deploy".into(),
        namepath: "ops::deploy".into(),
        doc: Some("ship\twithout\nparsing $(oops)".into()),
        body: vec!["echo safe".into()],
        ..Recipe::default()
    };
    SessionState::new(
        Invocation::new(PathBuf::from("/tmp/project"), None, None, false),
        Project {
            recipes: vec![recipe],
            warnings: vec![],
        },
    )
    .expect("session state")
}

#[test]
fn source_rows_have_opaque_ids_and_single_lines() {
    let state = state();
    let output = television::source_output(&state).unwrap();
    let fields: Vec<_> = output.trim_end().split('\t').collect();
    assert_eq!(fields.len(), 3);
    validate_id(fields[0]).unwrap();
    assert_eq!(fields[1], "ops::deploy");
    assert!(fields[2].contains("ship without parsing $(oops)"));
    assert!(!fields[0].contains("deploy"));
}

#[test]
fn invalid_ids_are_rejected_before_lookup() {
    let state = state();
    for id in [
        "ops::deploy",
        "jtv-0000000;",
        "JTV-00000000",
        "jtv-00000000 extra",
    ] {
        assert!(state.resolve(id).is_err(), "accepted {id:?}");
    }
}

#[test]
fn session_is_private_persistent_and_removed_on_drop() {
    let state = state();
    let session = SessionFile::create(&state).unwrap();
    let path = session.path().to_path_buf();
    assert_eq!(
        load(&path).unwrap().resolve("jtv-00000000").unwrap(),
        "ops::deploy"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(session);
    assert!(!path.exists());
}

#[test]
fn preview_uses_only_resolved_recipe_data() {
    let state = state();
    let preview = television::preview(&state, "jtv-00000000").unwrap();
    assert!(preview.contains("ops::deploy"));
    assert!(preview.contains("echo safe"));
    assert!(television::preview(&state, "ops::deploy").is_err());
}

#[test]
fn embedded_channel_has_constant_safe_templates() {
    let asset = jtv::channel::CHANNEL_CONTENT;
    assert!(asset.contains("command = \"jtv __tv-source\""));
    assert!(asset.contains("command = \"jtv __tv-preview {split:\\t:0}\""));
    assert!(asset.contains("command = \"jtv __tv-run {split:\\n:..|map:{split:\\t:0}|join: }\""));
    assert!(asset.contains("separator = \"\\n\""));
    assert!(!asset.contains("$JTV_SESSION"));
    assert!(!asset.contains("sh -c"));
}

#[test]
fn launch_command_passes_paths_as_arguments_and_only_session_as_environment() {
    let state = state();
    let session = SessionFile::create(&state).unwrap();
    let command = television::command(
        std::path::Path::new("tv"),
        std::path::Path::new("/tmp/cable with spaces"),
        std::path::Path::new("/tmp/project;literal"),
        &session,
    );
    let args: Vec<_> = command.get_args().map(|arg| arg.to_owned()).collect();
    assert_eq!(args[0], "jtv-recipes");
    assert_eq!(args[1], "/tmp/project;literal");
    assert_eq!(args[2], "--cable-dir");
    assert_eq!(args[3], "/tmp/cable with spaces");
    assert!(
        command
            .get_envs()
            .any(|(key, value)| key == "JTV_SESSION" && value.is_some())
    );
}

#[test]
fn television_0159_is_supported() {
    let version = semver::Version::parse("0.15.9").unwrap();
    assert!(television::version_is_supported(&version));
    assert!(!television::version_is_supported(
        &semver::Version::parse("0.15.8").unwrap()
    ));
}
