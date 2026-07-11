use std::fs;

use jtv::{
    model::{Project, Recipe},
    target::{TargetKind, file_candidates, has_discoverable_root, resolve},
};
use tempfile::tempdir;

fn project(namepaths: &[&str]) -> Project {
    Project {
        recipes: namepaths
            .iter()
            .map(|namepath| Recipe {
                namepath: (*namepath).into(),
                ..Recipe::default()
            })
            .collect(),
        ..Project::default()
    }
}

fn touch(root: &std::path::Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "# fixture\n").unwrap();
}

#[test]
fn constructs_only_the_four_approved_candidates_in_order() {
    assert_eq!(
        file_candidates("docker").unwrap(),
        [
            "docker.just",
            "docker/justfile",
            "justfiles/docker.just",
            "justfiles/docker/justfile",
        ]
        .map(std::path::PathBuf::from)
    );
}

#[test]
fn root_module_wins_and_all_overlaps_are_reported_in_order() {
    let root = tempdir().unwrap();
    for path in [
        "docker.just",
        "docker/justfile",
        "justfiles/docker.just",
        "justfiles/docker/justfile",
    ] {
        touch(root.path(), path);
    }
    let resolved = resolve(root.path(), "docker", Some(&project(&["docker::build"]))).unwrap();
    assert_eq!(resolved.selected, TargetKind::Module("docker".into()));
    assert_eq!(
        resolved.matches,
        [
            "module docker",
            "docker.just",
            "docker/justfile",
            "justfiles/docker.just",
            "justfiles/docker/justfile",
        ]
    );
    assert_eq!(
        resolved.warning().unwrap(),
        "WARNING: 'docker' resolves to multiple targets:\n  module docker\n  docker.just\n  docker/justfile\n  justfiles/docker.just\n  justfiles/docker/justfile"
    );
}

#[test]
fn first_existing_file_wins_without_unapproved_aliases() {
    let root = tempdir().unwrap();
    touch(root.path(), "docker/mod.just");
    touch(root.path(), "just.docker");
    touch(root.path(), "justfiles/docker.just");
    let resolved = resolve(root.path(), "docker", None).unwrap();
    assert_eq!(
        resolved.selected,
        TargetKind::Justfile("justfiles/docker.just".into())
    );
    assert_eq!(resolved.matches, ["justfiles/docker.just"]);
    assert_eq!(resolved.warning(), None);
}

#[test]
fn each_approved_standalone_layout_resolves_when_it_is_the_only_target() {
    for expected in [
        "docker.just",
        "docker/justfile",
        "justfiles/docker.just",
        "justfiles/docker/justfile",
    ] {
        let root = tempdir().unwrap();
        touch(root.path(), expected);
        let resolved = resolve(root.path(), "docker", None).unwrap();
        assert_eq!(
            resolved.selected,
            TargetKind::Justfile(expected.into()),
            "wrong selection for {expected}"
        );
        assert_eq!(resolved.matches, [expected]);
        assert_eq!(resolved.warning(), None);
    }
}

#[test]
fn unsupported_aliases_do_not_resolve_a_target() {
    let root = tempdir().unwrap();
    for path in [
        "just.docker",
        "docker/mod.just",
        "docker/.justfile",
        "justfiles/docker/mod.just",
        "justfiles/docker/.justfile",
    ] {
        touch(root.path(), path);
    }
    assert!(resolve(root.path(), "docker", None).is_err());
}

#[cfg(unix)]
#[test]
fn standalone_resolution_does_not_follow_directory_symlinks() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    touch(outside.path(), "justfile");
    symlink(outside.path(), root.path().join("docker")).unwrap();
    assert!(resolve(root.path(), "docker", None).is_err());
}

#[test]
fn nested_module_recipe_counts_but_similarly_named_module_does_not() {
    let root = tempdir().unwrap();
    let nested = resolve(
        root.path(),
        "docker",
        Some(&project(&["docker::ci::build"])),
    )
    .unwrap();
    assert_eq!(nested.selected, TargetKind::Module("docker".into()));
    assert!(resolve(root.path(), "dock", Some(&project(&["docker::build"]))).is_err());
}

#[test]
fn invalid_and_missing_names_have_stable_diagnostics() {
    let root = tempdir().unwrap();
    for name in ["", ".", "..", "a/b", r"a\b"] {
        assert!(
            file_candidates(name)
                .unwrap_err()
                .to_string()
                .contains("invalid target name")
        );
    }
    let error = resolve(root.path(), "docker", None)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "target 'docker' was not found; searched:\n  docker.just\n  docker/justfile\n  justfiles/docker.just\n  justfiles/docker/justfile"
    );
}

#[test]
fn root_discovery_searches_parents_case_insensitively() {
    let root = tempdir().unwrap();
    let child = root.path().join("a/b");
    fs::create_dir_all(&child).unwrap();
    touch(root.path(), "JustFile");
    assert!(has_discoverable_root(&child));
}

#[cfg(unix)]
#[test]
fn root_discovery_accepts_a_symlinked_justfile() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    touch(root.path(), "actual.just");
    symlink(
        root.path().join("actual.just"),
        root.path().join("justfile"),
    )
    .unwrap();
    assert!(has_discoverable_root(root.path()));
}
