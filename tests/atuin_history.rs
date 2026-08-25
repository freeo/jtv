//! Isolated compatibility proof for Atuin's public history lifecycle.

use std::{fs, path::PathBuf, process::Command, time::Duration};

use jtv::history::AtuinClient;

const CHILD_MARKER: &str = "JTV_REAL_ATUIN_TEST_CHILD";

#[test]
#[ignore = "requires the pinned local Atuin CLI"]
fn real_atuin_records_command_cwd_status_and_duration() {
    if std::env::var_os(CHILD_MARKER).is_some() {
        let cwd = PathBuf::from(std::env::var_os("JTV_REAL_ATUIN_CWD").unwrap());
        let client = AtuinClient::from_environment().expect("active wrapper contract");
        let outer = client
            .start("jtv", &cwd)
            .expect("outer Atuin history start");
        let entry = client
            .start(
                "just --justfile '/tmp/a b/justfile' deploy 'semi;colon'",
                &cwd,
            )
            .expect("Atuin history start");
        entry.finish(17, Duration::from_millis(42));
        outer.finish(0, Duration::from_millis(50));
        return;
    }

    let version = Command::new("atuin")
        .arg("--version")
        .output()
        .expect("Atuin 18.16.1 must be installed for this ignored contract");
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "atuin 18.16.1 (NO_GIT)"
    );

    let sandbox = tempfile::tempdir().unwrap();
    let config = sandbox.path().join("config");
    let data = sandbox.path().join("data");
    let home = sandbox.path().join("home");
    let cwd = sandbox.path().join("project with spaces");
    for directory in [&config, &data, &home, &cwd] {
        fs::create_dir_all(directory).unwrap();
    }

    let common_env = [
        ("ATUIN_CONFIG_DIR", config.as_os_str()),
        ("XDG_DATA_HOME", data.as_os_str()),
        ("HOME", home.as_os_str()),
        ("ATUIN_SESSION", std::ffi::OsStr::new("jtv-real-proof")),
    ];
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "real_atuin_records_command_cwd_status_and_duration",
        ])
        .envs(common_env)
        .env(CHILD_MARKER, "1")
        .env("JTV_REAL_ATUIN_CWD", &cwd)
        .env("JTV_SHELL_INTEGRATION", "zsh")
        .env("JTV_HISTORY_PROTOCOL", "1")
        .env("JTV_HISTORY_SESSION", "jtv-history.real-atuin-test")
        .env("JTV_ATUIN_BIN", "/usr/bin/atuin")
        .status()
        .unwrap();
    assert!(child.success());

    let history = Command::new("atuin")
        .args([
            "history",
            "list",
            "--format",
            "{command}|{directory}|{exit}|{duration}|{session}",
        ])
        .envs(common_env)
        .output()
        .unwrap();
    assert!(history.status.success());
    let history = String::from_utf8(history.stdout).unwrap();
    let rows = history.lines().collect::<Vec<_>>();
    assert_eq!(rows.len(), 2, "history={history:?}");
    assert!(
        rows.contains(&format!("jtv|{}|0|50ms|jtv-real-proof", cwd.display()).as_str()),
        "history={history:?}"
    );
    assert!(
        rows.contains(
            &format!(
                "just --justfile '/tmp/a b/justfile' deploy 'semi;colon'|{}|17|42ms|jtv-real-proof",
                cwd.display()
            )
            .as_str()
        ),
        "history={history:?}"
    );
}
