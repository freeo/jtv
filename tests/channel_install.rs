use std::fs;

use jtv::channel::{self, InstallOutcome};

#[test]
fn install_is_atomic_and_idempotent() {
    let root = tempfile::tempdir().unwrap();
    let first = channel::install(root.path(), false).unwrap();
    assert!(matches!(first, InstallOutcome::Installed { .. }));
    assert_eq!(
        fs::read_to_string(channel::channel_path(root.path())).unwrap(),
        channel::CHANNEL_CONTENT
    );

    let second = channel::install(root.path(), false).unwrap();
    assert!(matches!(second, InstallOutcome::AlreadyCurrent { .. }));
}

#[test]
fn conflict_is_refused_unless_force_backs_it_up() {
    let root = tempfile::tempdir().unwrap();
    let path = channel::channel_path(root.path());
    fs::write(&path, "user-owned channel").unwrap();
    let error = channel::install(root.path(), false).unwrap_err();
    assert!(error.to_string().contains("--force"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "user-owned channel");

    let outcome = channel::install(root.path(), true).unwrap();
    let InstallOutcome::Replaced { backup, .. } = outcome else {
        panic!("expected replacement")
    };
    assert_eq!(fs::read_to_string(backup).unwrap(), "user-owned channel");
    assert_eq!(fs::read_to_string(path).unwrap(), channel::CHANNEL_CONTENT);
}
