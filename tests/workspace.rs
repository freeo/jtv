use std::{fs, path::Path};

use jtv::{just::parse_project_dump, workspace::discover};
use tempfile::tempdir;

fn touch(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "# fixture\n").unwrap();
}

#[test]
fn discovers_exact_names_with_deterministic_labels_and_order() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    for path in [
        "justfile",
        "docker.just",
        "supabase/justfile",
        "supabase/db.just",
        "tools/.justfile",
        "upper/JUSTFILE",
        "ignored.JUST",
        "notjust",
    ] {
        touch(root, path);
    }

    let found = discover(root, &root.join("justfile"), &[]);
    let rows: Vec<_> = found
        .justfiles
        .iter()
        .map(|item| {
            (
                item.relative_path.to_string_lossy().replace('\\', "/"),
                item.label.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("docker.just".into(), "docker.just"),
            ("supabase/db.just".into(), "supabase/db.just"),
            ("supabase/justfile".into(), "supabase/"),
            ("tools/.justfile".into(), "tools/"),
            ("upper/JUSTFILE".into(), "upper/"),
        ]
    );
    assert!(found.warnings.is_empty());
}

#[test]
fn respects_gitignore_and_fixed_heavy_tree_exclusions() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    touch(root, "justfile");
    touch(root, "kept/tool.just");
    touch(root, "generated/hidden.just");
    touch(root, "target/build.just");
    touch(root, "node_modules/pkg.just");
    touch(root, ".venv/env.just");
    fs::write(root.join(".gitignore"), "generated/\n").unwrap();
    let found = discover(root, &root.join("justfile"), &[]);
    let paths: Vec<_> = found
        .justfiles
        .iter()
        .map(|item| item.relative_path.to_string_lossy().to_string())
        .collect();
    assert_eq!(paths, ["kept/tool.just"]);
}

#[test]
fn excludes_authoritative_module_sources_after_canonicalization() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    touch(root, "justfile");
    touch(root, "docker.just");
    touch(root, "other.just");
    let found = discover(root, &root.join("justfile"), &[root.join("./docker.just")]);
    assert_eq!(found.justfiles.len(), 1);
    assert_eq!(found.justfiles[0].label, "other.just");
}

#[test]
fn larger_workspace_is_complete_and_stably_sorted() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    touch(root, "justfile");
    for index in (0..128).rev() {
        touch(root, &format!("services/service-{index:03}/tasks.just"));
    }
    let first = discover(root, &root.join("justfile"), &[]);
    let second = discover(root, &root.join("justfile"), &[]);
    assert_eq!(first.justfiles.len(), 128);
    assert_eq!(first.justfiles, second.justfiles);
    assert_eq!(first.justfiles[0].label, "services/service-000/tasks.just");
    assert_eq!(
        first.justfiles[127].label,
        "services/service-127/tasks.just"
    );
}

#[test]
fn extracts_nested_module_source_paths_from_just_json() {
    let parsed = parse_project_dump(
        br#"{
          "recipes": {},
          "source": "/workspace/justfile",
          "modules": {
            "docker": {
              "source": "/workspace/docker.just",
              "module_path": "docker",
              "recipes": {},
              "modules": {
                "release": {
                  "source": "/workspace/release.just",
                  "module_path": "docker::release",
                  "recipes": {}
                }
              }
            }
          }
        }"#,
        None,
    )
    .unwrap();
    assert_eq!(
        parsed.module_sources,
        [
            std::path::PathBuf::from("/workspace/docker.just"),
            std::path::PathBuf::from("/workspace/release.just")
        ]
    );
}

#[cfg(unix)]
#[test]
fn never_traverses_directory_symlinks_or_accepts_file_symlinks() {
    use std::os::unix::fs::symlink;
    let temp = tempdir().unwrap();
    let outside = tempdir().unwrap();
    touch(temp.path(), "justfile");
    touch(outside.path(), "escaped.just");
    symlink(outside.path(), temp.path().join("linked-dir")).unwrap();
    symlink(
        outside.path().join("escaped.just"),
        temp.path().join("linked.just"),
    )
    .unwrap();
    let found = discover(temp.path(), &temp.path().join("justfile"), &[]);
    assert!(found.justfiles.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn skips_non_utf8_candidates_with_a_stable_warning() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let temp = tempdir().unwrap();
    touch(temp.path(), "justfile");
    let name = OsString::from_vec(vec![0xff, b'.', b'j', b'u', b's', b't']);
    fs::write(temp.path().join(name), "# fixture\n").unwrap();
    let found = discover(temp.path(), &temp.path().join("justfile"), &[]);
    assert!(found.justfiles.is_empty());
    assert_eq!(found.warnings.len(), 1);
    assert_eq!(found.warnings[0].message, "non-UTF-8 path is not supported");
}
