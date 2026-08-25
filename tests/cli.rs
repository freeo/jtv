use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn help_and_version_describe_the_rust_application() {
    Command::cargo_bin("jtv")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Interactive Justfile runner powered by Television",
        ))
        .stdout(predicate::str::contains("[NAME]"))
        .stdout(predicate::str::contains("doctor"));

    Command::cargo_bin("jtv")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("jtv 0.4.0"));
}

#[test]
fn positional_target_conflicts_with_explicit_targeting_flags() {
    for flag in ["--justfile", "--module"] {
        Command::cargo_bin("jtv")
            .unwrap()
            .args(["docker", flag, "explicit"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("cannot be used with"));
    }
}

#[test]
fn init_is_idempotent_in_an_isolated_cable_directory() {
    let temp = tempdir().unwrap();
    let cable = temp.path().join("cable");

    Command::cargo_bin("jtv")
        .unwrap()
        .env("JTV_TV_CABLE_DIR", &cable)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed"));

    Command::cargo_bin("jtv")
        .unwrap()
        .env("JTV_TV_CABLE_DIR", &cable)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("already current"));
}

#[test]
fn picker_source_emits_only_opaque_ids_and_sanitized_display() {
    let temp = tempdir().unwrap();
    let state = temp.path().join("picker.json");
    fs::write(
        &state,
        r#"{"entries":[{"id":"pick-00000000","display":"hello\tworld\nline","value":"literal"}]}"#,
    )
    .unwrap();

    Command::cargo_bin("jtv")
        .unwrap()
        .env("JTV_PICKER_STATE", &state)
        .arg("__picker-source")
        .assert()
        .success()
        .stdout("pick-00000000\thello world line\n");
}

#[cfg(unix)]
mod unix {
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    use assert_cmd::Command;
    use predicates::prelude::*;
    use tempfile::tempdir;

    fn script(path: &Path, body: &str) {
        fs::write(path, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn doctor_checks_versions_and_the_installed_channel() {
        let temp = tempdir().unwrap();
        let cable = temp.path().join("cable");
        let just = temp.path().join("just");
        let tv = temp.path().join("tv");
        script(
            &just,
            "if [ \"${1:-}\" = --version ]; then printf 'just 1.53.0\\n'; else cat >/dev/null; printf '%s\\n' '{\"recipes\":{\"probe\":{\"name\":\"probe\",\"namepath\":\"probe\"}}}'; fi",
        );
        script(&tv, "printf 'television 0.15.9\\n'");

        Command::cargo_bin("jtv")
            .unwrap()
            .env("JTV_TV_CABLE_DIR", &cable)
            .arg("init")
            .assert()
            .success();

        Command::cargo_bin("jtv")
            .unwrap()
            .env("JTV_TV_CABLE_DIR", &cable)
            .env("JTV_JUST", &just)
            .env("JTV_TV", &tv)
            .arg("doctor")
            .assert()
            .success()
            .stdout(predicate::str::contains("[OK] just 1.53.0"))
            .stdout(predicate::str::contains("[OK] television 0.15.9"))
            .stdout(predicate::str::contains("[OK] channel"))
            .stdout(predicate::str::contains(
                "shell history integration inactive",
            ));

        Command::cargo_bin("jtv")
            .unwrap()
            .env("JTV_TV_CABLE_DIR", &cable)
            .env("JTV_JUST", &just)
            .env("JTV_TV", &tv)
            .env("JTV_SHELL_INTEGRATION", "zsh")
            .env("JTV_HISTORY_PROTOCOL", "1")
            .env("JTV_HISTORY_SESSION", "jtv-history.doctor-test")
            .env("JTV_ZSH_AUTOSUGGESTIONS", "1")
            .env("JTV_ATUIN_BIN", "/usr/bin/atuin")
            .env("ATUIN_SESSION", "doctor-session")
            .arg("doctor")
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "[OK] shell history integration zsh",
            ))
            .stdout(predicate::str::contains(
                "[OK] zsh-autosuggestions history strategy detected",
            ))
            .stdout(predicate::str::contains(
                "[OK] Atuin shell history lifecycle enabled",
            ))
            .stdout(predicate::str::contains(
                "configured secret parameters are silently omitted",
            ));
    }

    #[test]
    fn top_level_launch_uses_the_channel_and_private_session() {
        let temp = tempdir().unwrap();
        let cable = temp.path().join("cable");
        let tv = temp.path().join("tv");
        let record = temp.path().join("tv-record");
        script(
            &tv,
            &format!(
                "if [ \"${{1:-}}\" = --version ]; then printf 'television 0.15.9\\n'; else printf '%s\\n' \"$*\" > '{}'; printf '%s\\n' \"$JTV_SESSION\" >> '{}'; fi",
                record.display(),
                record.display()
            ),
        );

        Command::cargo_bin("jtv")
            .unwrap()
            .env("JTV_TV_CABLE_DIR", &cable)
            .arg("init")
            .assert()
            .success();

        Command::cargo_bin("jtv")
            .unwrap()
            .env("JTV_TV_CABLE_DIR", &cable)
            .env("JTV_TV", &tv)
            .arg("--justfile")
            .arg("tests/fixtures/e2e/justfile")
            .assert()
            .success();

        let recorded = fs::read_to_string(record).unwrap();
        assert!(recorded.contains("jtv-recipes"));
        assert!(recorded.contains("--cable-dir"));
        assert!(recorded.contains("--no-remote"));
        assert!(recorded.lines().nth(1).is_some_and(|path| !path.is_empty()));
    }

    #[test]
    fn named_target_prefers_root_module_and_warns_after_television_exits() {
        let temp = tempdir().unwrap();
        let cable = temp.path().join("cable");
        let just = temp.path().join("just-bin");
        let tv = temp.path().join("tv-bin");
        fs::write(temp.path().join("justfile"), "mod docker\n").unwrap();
        fs::write(temp.path().join("docker.just"), "build:\n  true\n").unwrap();
        script(
            &just,
            "if [ \"${1:-}\" = --version ]; then printf 'just 1.53.0\\n'; else printf '%s\\n' '{\"recipes\":{\"probe\":{\"name\":\"probe\",\"namepath\":\"probe\"}},\"modules\":{\"docker\":{\"module_path\":\"docker\",\"recipes\":{\"build\":{\"name\":\"build\",\"namepath\":\"docker::build\"}}}}}'; fi",
        );
        script(
            &tv,
            "if [ \"${1:-}\" = --version ]; then printf 'television 0.15.9\\n'; else printf 'TV EXITED\\n'; fi",
        );

        Command::cargo_bin("jtv")
            .unwrap()
            .env("JTV_TV_CABLE_DIR", &cable)
            .arg("init")
            .assert()
            .success();

        Command::cargo_bin("jtv")
            .unwrap()
            .current_dir(temp.path())
            .env("JTV_TV_CABLE_DIR", &cable)
            .env("JTV_JUST", &just)
            .env("JTV_TV", &tv)
            .arg("docker")
            .assert()
            .success()
            .stdout(predicate::eq(
                "TV EXITED\nWARNING: 'docker' resolves to multiple targets:\n  module docker\n  docker.just\n",
            ));
    }

    #[test]
    fn invalid_child_justfile_warns_only_after_television_exits() {
        let temp = tempdir().unwrap();
        let cable = temp.path().join("cable");
        let just = temp.path().join("just-bin");
        let tv = temp.path().join("tv-bin");
        let root = temp.path().join("justfile");
        fs::write(&root, "root:\n  true\n").unwrap();
        fs::write(temp.path().join("broken.just"), "not valid just syntax").unwrap();
        script(
            &just,
            &format!(
                "if [ \"${{1:-}}\" = --version ]; then printf 'just 1.53.0\\n'; exit; fi\ncase \" $* \" in *' - '*) printf '%s\\n' '{{\"recipes\":{{\"probe\":{{\"name\":\"probe\",\"namepath\":\"probe\"}}}}}}'; exit;; *' --justfile {}/broken.just '*) printf 'broken child\\n' >&2; exit 9;; esac\nprintf '%s\\n' '{{\"source\":\"{}\",\"recipes\":{{\"root\":{{\"name\":\"root\",\"namepath\":\"root\"}}}}}}'",
                temp.path().display(),
                root.display()
            ),
        );
        script(
            &tv,
            "if [ \"${1:-}\" = --version ]; then printf 'television 0.15.9\\n'; else printf 'TV EXITED\\n'; fi",
        );

        Command::cargo_bin("jtv")
            .unwrap()
            .env("JTV_TV_CABLE_DIR", &cable)
            .arg("init")
            .assert()
            .success();

        Command::cargo_bin("jtv")
            .unwrap()
            .current_dir(temp.path())
            .env("JTV_TV_CABLE_DIR", &cable)
            .env("JTV_JUST", &just)
            .env("JTV_TV", &tv)
            .assert()
            .success()
            .stdout(predicate::str::starts_with("TV EXITED\nWARNING:"))
            .stdout(predicate::str::contains("broken.just"))
            .stdout(predicate::str::contains("broken child"));
    }
}
