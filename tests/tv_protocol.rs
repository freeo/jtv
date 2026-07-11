use std::path::PathBuf;

use jtv::{
    config::Config,
    invocation::Invocation,
    model::{Project, Recipe},
    presentation::{PresentationOptions, ResolvedColorMode, ResolvedIconMode},
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
    let output = television::source_output(&state, television::SourceView::Root).unwrap();
    let fields: Vec<_> = output.trim_end().split('\t').collect();
    assert_eq!(fields.len(), 3);
    validate_id(fields[0]).unwrap();
    assert_eq!(fields[1], "[recipe] ops::deploy");
    assert!(fields[2].contains("ship without parsing $(oops)"));
    assert!(!fields[0].contains("deploy"));
}

#[test]
fn styled_source_rows_keep_ansi_only_in_display_and_opaque_ids_in_output() {
    let base = state();
    let primary = base.primary_target().unwrap();
    let state = SessionState::new_with_presentation(
        primary.invocation.clone(),
        primary.project.clone(),
        Config::default(),
        PresentationOptions {
            color: ResolvedColorMode::Color,
            source_color: ResolvedColorMode::Color,
            icons: ResolvedIconMode::Unicode,
            compact: false,
        },
    )
    .unwrap();
    let output = television::source_output(&state, television::SourceView::Root).unwrap();
    let fields: Vec<_> = output.trim_end().split('\t').collect();
    assert_eq!(fields.len(), 3);
    validate_id(fields[0]).unwrap();
    assert!(fields[1].contains("▶"));
    assert!(fields[1].contains("\x1b[0;36mops::deploy\x1b[0m"));
    assert!(!fields[0].contains('\x1b'));
    assert!(!fields[2].contains('\x1b'));
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
        load(&path)
            .unwrap()
            .resolve(state.catalog.selections.keys().next().unwrap())
            .unwrap()
            .recipe
            .namepath,
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
    let id = state.catalog.selections.keys().next().unwrap();
    let preview = television::preview(&state, id).unwrap();
    assert!(preview.contains("ops::deploy"));
    assert!(preview.contains("echo safe"));
    assert!(television::preview(&state, "ops::deploy").is_err());
}

#[test]
fn embedded_channel_has_constant_safe_templates() {
    let asset = jtv::channel::CHANNEL_CONTENT;
    let channel: toml::Value = toml::from_str(asset).expect("embedded channel parses as TOML");
    assert_eq!(channel["source"]["ansi"].as_bool(), Some(true));
    assert_eq!(channel["source"]["display"].as_str(), Some("{split:\t:1}"));
    assert_eq!(channel["source"]["output"].as_str(), Some("{split:\t:0}"));
    let previews = channel["preview"]["command"].as_array().unwrap();
    assert_eq!(previews.len(), 2);
    assert_eq!(channel["actions"]["run"]["mode"].as_str(), Some("execute"));
    assert_eq!(
        channel["actions"]["dry-run"]["mode"].as_str(),
        Some("execute")
    );
    let commands = channel["source"]["command"].as_array().unwrap();
    assert_eq!(commands.len(), 4);
    assert_eq!(commands[0]["name"].as_str(), Some("Root"));
    assert_eq!(
        commands[0]["run"].as_str(),
        Some("jtv __tv-source --view root")
    );
    assert_eq!(commands[1]["name"].as_str(), Some("Subfolders"));
    assert_eq!(commands[2]["name"].as_str(), Some("Modules"));
    assert_eq!(commands[3]["name"].as_str(), Some("All"));
    assert!(asset.contains("ansi = true"));
    assert!(asset.contains("\"jtv __tv-preview {split:\\t:0}\""));
    assert!(asset.contains("jtv __tv-preview --definition {split:\\t:0}"));
    assert!(asset.contains("command = \"jtv __tv-run {split:\\n:..|map:{split:\\t:0}|join: }\""));
    assert!(asset.contains("command = \"jtv __tv-run --dry-run"));
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
        &state.presentation,
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
fn compact_launch_overrides_the_channel_with_portrait_layout() {
    let mut state = state();
    state.presentation.compact = true;
    let session = SessionFile::create(&state).unwrap();
    let command = television::command(
        std::path::Path::new("tv"),
        std::path::Path::new("/tmp/cable"),
        std::path::Path::new("/tmp/project"),
        &session,
        &state.presentation,
    );
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(args.windows(2).any(|pair| pair == ["--layout", "portrait"]));
    assert!(args.windows(2).any(|pair| pair == ["--preview-size", "50"]));
}

#[test]
fn television_0159_is_supported() {
    let version = semver::Version::parse("0.15.9").unwrap();
    assert!(television::version_is_supported(&version));
    assert!(!television::version_is_supported(
        &semver::Version::parse("0.15.8").unwrap()
    ));
}
