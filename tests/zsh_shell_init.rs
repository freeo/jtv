#![cfg(unix)]

use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
};

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::TempDir;

fn write_executable(path: &Path, source: &str) {
    fs::write(path, source).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn autosuggestions_plugin() -> Option<std::path::PathBuf> {
    let mut candidates = vec![
        std::path::PathBuf::from("/usr/share/zsh-autosuggestions/zsh-autosuggestions.zsh"),
        std::path::PathBuf::from(
            "/usr/share/zsh/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh",
        ),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(Path::new(&home).join(
            ".local/share/zinit/plugins/zsh-users---zsh-autosuggestions/zsh-autosuggestions.zsh",
        ));
    }
    candidates.into_iter().find(|path| path.is_file())
}

#[test]
fn shell_init_zsh_prints_sourceable_static_adapter() {
    let output = cargo_bin_cmd!("jtv")
        .args(["shell-init", "zsh"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let source = String::from_utf8(output.stdout).unwrap();
    assert!(source.contains("function jtv()"));
    assert!(source.contains("JTV_HISTORY_SINK=$sink"));
    assert!(source.contains("JTV_SHELL_INTEGRATION=zsh"));
    assert!(source.contains("JTV_HISTORY_PROTOCOL=1"));
    assert!(source.contains("JTV_HISTORY_SESSION=$history_session"));
    assert!(source.contains("JTV_ATUIN_BIN=${commands[atuin]}"));
    assert!(source.contains("JTV_ZSH_AUTOSUGGESTIONS=1"));
    assert!(!source.contains(">>"));
}

#[test]
fn wrapper_preserves_streams_arguments_status_and_imports_literal_history() {
    if Command::new("zsh").arg("--version").output().is_err() {
        eprintln!("zsh unavailable; skipping adapter execution contract");
        return;
    }
    let sandbox = TempDir::new().unwrap();
    let fake_bin = sandbox.path().join("bin");
    fs::create_dir(&fake_bin).unwrap();
    write_executable(
        &fake_bin.join("jtv"),
        r##"#!/bin/sh
printf 'fake stdout\n'
printf 'fake stderr\n' >&2
printf '%s\n' "$#:$1:$2" > "$JTV_CAPTURE"
printf '%s\n' "just --justfile '/project/a b.just' deploy '\$HOME; touch /tmp/nope'" > "$JTV_HISTORY_SINK"
exit 37
"##,
    );
    let init = cargo_bin_cmd!("jtv")
        .args(["shell-init", "zsh"])
        .output()
        .unwrap();
    let init_path = sandbox.path().join("init.zsh");
    fs::write(&init_path, init.stdout).unwrap();
    let capture = sandbox.path().join("capture");
    let history = sandbox.path().join("history");
    let script = format!(
        r#"source {init}
HISTFILE={history}
SAVEHIST=100
jtv 'a b' '$HOME; false'
result=$?
print -r -- "STATUS:$result"
print -r -- 'LIVE-BEGIN'
fc -l -1
print -r -- 'LIVE-END'
exit 0
"#,
        init = init_path.display(),
        history = history.display(),
    );
    let output = Command::new("zsh")
        .arg("-f")
        .arg("-i")
        .arg("-c")
        .arg(script)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("JTV_CAPTURE", &capture)
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        output.status.success(),
        "status={:?} stdout={stdout:?} stderr={stderr:?}",
        output.status.code()
    );
    assert!(stdout.contains("fake stdout\n"), "{stdout:?}");
    assert!(stdout.contains("STATUS:37\n"), "{stdout:?}");
    assert!(stdout.contains("just --justfile '/project/a b.just' deploy '$HOME; touch /tmp/nope'"));
    assert_eq!(stderr, "fake stderr\n");
    assert_eq!(fs::read_to_string(capture).unwrap(), "2:a b:$HOME; false\n");
    assert_eq!(
        fs::read_to_string(history).unwrap(),
        "just --justfile '/project/a b.just' deploy '$HOME; touch /tmp/nope'\n"
    );
    assert!(!Path::new("/tmp/nope").exists());
}

#[test]
fn wrapper_entries_are_visible_to_zsh_autosuggestions_history_strategy() {
    let Some(plugin) = autosuggestions_plugin() else {
        eprintln!("zsh-autosuggestions unavailable; skipping plugin contract");
        return;
    };
    let sandbox = TempDir::new().unwrap();
    let fake_bin = sandbox.path().join("bin");
    fs::create_dir(&fake_bin).unwrap();
    write_executable(
        &fake_bin.join("jtv"),
        "#!/bin/sh\nprintf '%s\\n' \"just --justfile '/project/a b.just' deploy\" > \"$JTV_HISTORY_SINK\"\n",
    );
    let init = cargo_bin_cmd!("jtv")
        .args(["shell-init", "zsh"])
        .output()
        .unwrap();
    let init_path = sandbox.path().join("init.zsh");
    fs::write(&init_path, init.stdout).unwrap();
    for sources in [
        format!(
            "source {}\nsource {}",
            init_path.display(),
            plugin.display()
        ),
        format!(
            "source {}\nsource {}",
            plugin.display(),
            init_path.display()
        ),
    ] {
        let script = format!(
            "{sources}\njtv\n_zsh_autosuggest_strategy_history 'just --justfile'\nprint -r -- \"SUGGESTION:$suggestion\"\nexit\n"
        );
        let mut child = Command::new("zsh")
            .args(["-f", "-i"])
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    fake_bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(script.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("SUGGESTION:just --justfile '/project/a b.just' deploy"),
            "sources={sources:?} stdout={stdout:?}"
        );
    }
}

#[test]
fn wrapper_signal_path_drops_records_and_removes_private_sink() {
    let sandbox = TempDir::new().unwrap();
    let fake_bin = sandbox.path().join("bin");
    let tmp = sandbox.path().join("tmp");
    fs::create_dir(&fake_bin).unwrap();
    fs::create_dir(&tmp).unwrap();
    write_executable(
        &fake_bin.join("jtv"),
        "#!/bin/sh\nprintf 'just must-not-survive\\n' > \"$JTV_HISTORY_SINK\"\nkill -TERM \"$PPID\"\nsleep 0.1\nexit 0\n",
    );
    let init = cargo_bin_cmd!("jtv")
        .args(["shell-init", "zsh"])
        .output()
        .unwrap();
    let init_path = sandbox.path().join("init.zsh");
    fs::write(&init_path, init.stdout).unwrap();
    let history = sandbox.path().join("history");
    let script = format!(
        "source '{}'\nHISTFILE='{}'; SAVEHIST=100\njtv\nprint -r -- STATUS:$?\nexit 0\n",
        init_path.display(),
        history.display()
    );
    let output = Command::new("zsh")
        .args(["-f", "-i", "-c", &script])
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("TMPDIR", &tmp)
        .output()
        .unwrap();
    assert!(output.status.success(), "output={output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("STATUS:143"));
    assert!(
        !history.exists()
            || !fs::read_to_string(&history)
                .unwrap()
                .contains("must-not-survive")
    );
    assert!(
        fs::read_dir(tmp)
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with("jtv-history."))
    );
}

#[test]
fn wrapper_persists_with_incremental_and_shared_history_modes() {
    let sandbox = TempDir::new().unwrap();
    let fake_bin = sandbox.path().join("bin");
    fs::create_dir(&fake_bin).unwrap();
    write_executable(
        &fake_bin.join("jtv"),
        "#!/bin/sh\nprintf 'just option-mode\\n' > \"$JTV_HISTORY_SINK\"\n",
    );
    let init = cargo_bin_cmd!("jtv")
        .args(["shell-init", "zsh"])
        .output()
        .unwrap();
    let init_path = sandbox.path().join("init.zsh");
    fs::write(&init_path, init.stdout).unwrap();
    for (name, option) in [
        ("default", ""),
        ("incremental", "setopt INC_APPEND_HISTORY"),
        ("shared", "setopt SHARE_HISTORY"),
    ] {
        let history = sandbox.path().join(format!("history-{name}"));
        let script = format!(
            "source '{}'; HISTFILE='{}'; SAVEHIST=100; HISTSIZE=100; {}; jtv; exit 0",
            init_path.display(),
            history.display(),
            option
        );
        let output = Command::new("zsh")
            .args(["-f", "-i", "-c", &script])
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    fake_bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .unwrap();
        assert!(output.status.success(), "mode={name} output={output:?}");
        assert!(
            fs::read_to_string(history)
                .unwrap()
                .contains("just option-mode"),
            "mode={name}"
        );
    }
}
