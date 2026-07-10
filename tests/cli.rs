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
        .stdout(predicate::str::contains("doctor"));

    Command::cargo_bin("jtv")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("jtv 0.4.0"));
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
            .stdout(predicate::str::contains("[OK] channel"));
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
}
