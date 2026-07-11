#![cfg(all(unix, target_os = "linux"))]
#![allow(dead_code)]

#[path = "support/keys.rs"]
mod keys;
#[path = "support/pty.rs"]
mod pty;
#[path = "support/screen.rs"]
mod screen;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use keys::Key;
use pty::{PtyCommand, PtySession};

const SCREEN_TIMEOUT: Duration = Duration::from_secs(10);
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Locks down the behavior jtv depends on before adopting Television's
/// multi-command source syntax. This is intentionally an isolated upstream
/// compatibility probe: it neither uses nor changes jtv production code.
#[test]
#[ignore = "requires Linux plus pinned television 0.15.9; run serialized"]
fn television_0159_multi_source_ctrl_s_contract() {
    let fixture = SourceCycleFixture::new().expect("create source-cycle fixture");
    let mut tv = fixture.launch().expect("launch pinned Television");

    let root = wait(&tv, "initial Root source", "root-one");
    assert!(
        root.contains("Root"),
        "named initial source must be visible"
    );
    assert!(!root.contains("folder-one"));

    tv.send_text("one").unwrap();
    wait(&tv, "Root query", "root-one");
    tv.send_key(Key::Ctrl('s')).unwrap();
    let subfolders = wait(&tv, "Subfolders source", "folder-one");
    assert!(subfolders.contains("Subfolders"));
    assert!(
        subfolders.contains("folder-one"),
        "query must survive source cycling"
    );
    assert!(
        !subfolders.contains("folder-two"),
        "surviving query filters"
    );

    tv.send_key(Key::Ctrl('s')).unwrap();
    let modules = wait(&tv, "Modules source", "module-one");
    assert!(modules.contains("Modules"));

    tv.send_key(Key::Ctrl('s')).unwrap();
    let empty = wait(&tv, "empty All source", "All");
    assert!(!empty.contains("root-one"));
    assert!(!empty.contains("folder-one"));

    tv.send_key(Key::Ctrl('s')).unwrap();
    let wrapped = wait(&tv, "wrapped Root source", "root-one");
    assert!(wrapped.contains("Root"), "source order must wrap to Root");

    tv.send_key(Key::Enter).unwrap();
    let status = tv.wait_for_exit(EXIT_TIMEOUT).expect("Television exits");
    assert_eq!(status.exit_code(), 0);

    let transcript = tv.transcript();
    let output = String::from_utf8_lossy(&transcript);
    assert!(output.contains("root-one"));

    assert_eq!(
        fixture.source_calls(),
        vec!["Root", "Subfolders", "Modules", "All", "Root"],
        "each initial/cycled source executes exactly once, in declaration order"
    );
    let previews = fixture.preview_calls();
    assert!(
        previews.iter().any(|line| line == "root-one")
            && previews.iter().any(|line| line == "folder-one")
            && previews.iter().any(|line| line == "module-one"),
        "preview callbacks must receive entries from every non-empty source: {previews:?}"
    );
}

#[test]
#[ignore = "requires Linux plus pinned television 0.15.9; run serialized"]
fn television_0159_preserves_marks_across_source_cycles() {
    let fixture = SourceCycleFixture::new().expect("create source-cycle fixture");
    let mut tv = fixture.launch().expect("launch pinned Television");
    wait(&tv, "initial Root source", "root-one");
    tv.send_text("one").unwrap();
    wait(&tv, "filtered Root", "root-one");
    tv.send_key(Key::Tab).unwrap();

    tv.send_key(Key::Ctrl('s')).unwrap();
    wait(&tv, "Subfolders source", "folder-one");
    tv.send_key(Key::Ctrl('s')).unwrap();
    let modules = wait(&tv, "Modules source", "module-one");

    assert!(modules.contains("module-one") && !modules.contains("folder-one"));
    assert_eq!(
        fixture.source_calls(),
        vec!["Root", "Subfolders", "Modules"],
        "a retained mark must not prevent later source callbacks"
    );
    tv.send_key(Key::Enter).unwrap();
    assert_eq!(tv.wait_for_exit(EXIT_TIMEOUT).unwrap().exit_code(), 0);
    let transcript = tv.transcript();
    let tail = &transcript[transcript.len().saturating_sub(256)..];
    assert!(
        String::from_utf8_lossy(tail).contains("root-one"),
        "Enter after cycling must return the marked Root entry"
    );
}

fn wait(tv: &PtySession, description: &str, needle: &str) -> screen::ScreenFrame {
    let frame = tv
        .wait_for_screen(description, SCREEN_TIMEOUT, |frame| frame.contains(needle))
        .unwrap_or_else(|error| panic!("{error}\n{}", tv.redacted_diagnostic()));
    // Television 0.15.9 keeps a 200 ms reload guard after each source change.
    // A second Ctrl-S inside that window advances the label but suppresses the
    // callback. Real typing naturally exceeds it; make the contract deterministic.
    thread::sleep(Duration::from_millis(250));
    frame
}

struct SourceCycleFixture {
    _root: tempfile::TempDir,
    root: PathBuf,
    xdg: PathBuf,
    source_log: PathBuf,
    preview_log: PathBuf,
    tv: PathBuf,
}

impl SourceCycleFixture {
    fn new() -> io::Result<Self> {
        let tv = pinned_tv()?;
        let temp = tempfile::tempdir()?;
        let root = temp.path().to_path_buf();
        let xdg = root.join("xdg");
        let cable = xdg.join("television/cable");
        fs::create_dir_all(&cable)?;
        let source_log = root.join("source.log");
        let preview_log = root.join("preview.log");
        fs::write(&source_log, [])?;
        fs::write(&preview_log, [])?;

        let source = root.join("source");
        executable(
            &source,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\ncase \"$1\" in\n  Root) printf 'root-one\\nroot-two\\n' ;;\n  Subfolders) printf 'folder-one\\nfolder-two\\n' ;;\n  Modules) printf 'module-one\\nmodule-two\\n' ;;\n  All) : ;;\nesac\n",
                source_log.display()
            ),
        )?;
        let preview = root.join("preview");
        executable(
            &preview,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\nprintf 'preview:%s\\n' \"$1\"\n",
                preview_log.display()
            ),
        )?;

        let channel = format!(
            r#"[metadata]
name = "jtv-source-cycle-contract"
description = "Pinned multi-source contract probe"
requirements = []

[source]
[[source.command]]
name = "Root"
run = "{} Root"
[[source.command]]
name = "Subfolders"
run = "{} Subfolders"
[[source.command]]
name = "Modules"
run = "{} Modules"
[[source.command]]
name = "All"
run = "{} All"

[preview]
command = "{} {{}}"

[ui]
layout = "landscape"
input_header = "Contract query"
input_prompt = "filter> "
"#,
            toml_path(&source),
            toml_path(&source),
            toml_path(&source),
            toml_path(&source),
            toml_path(&preview),
        );
        fs::write(cable.join("jtv-source-cycle-contract.toml"), channel)?;

        Ok(Self {
            _root: temp,
            root,
            xdg,
            source_log,
            preview_log,
            tv,
        })
    }

    fn launch(&self) -> io::Result<PtySession> {
        let mut env = BTreeMap::<OsString, OsString>::new();
        env.insert("HOME".into(), self.root.as_os_str().to_owned());
        env.insert("XDG_CONFIG_HOME".into(), self.xdg.as_os_str().to_owned());
        env.insert(
            "XDG_CACHE_HOME".into(),
            self.root.join("cache").into_os_string(),
        );
        env.insert(
            "XDG_DATA_HOME".into(),
            self.root.join("data").into_os_string(),
        );
        env.insert(
            "XDG_RUNTIME_DIR".into(),
            self.root.join("runtime").into_os_string(),
        );
        env.insert("PATH".into(), std::env::var_os("PATH").unwrap_or_default());
        fs::create_dir_all(self.root.join("runtime"))?;

        let mut command = PtyCommand::new(&self.tv, &self.root)
            .arg("jtv-source-cycle-contract")
            .viewport(120, 40);
        for (key, value) in env {
            command = command.env(key, value);
        }
        PtySession::spawn(command)
    }

    fn source_calls(&self) -> Vec<String> {
        lines(&self.source_log)
    }

    fn preview_calls(&self) -> Vec<String> {
        lines(&self.preview_log)
    }
}

fn pinned_tv() -> io::Result<PathBuf> {
    let path = std::env::var_os("JTV_TEST_REAL_TV")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/bin/tv"));
    let temp = tempfile::tempdir()?;
    let output = Command::new(&path)
        .arg("--version")
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .output()?;
    let actual = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if output.status.success() && actual == "television 0.15.9" {
        Ok(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("expected television 0.15.9, got {actual:?}"),
        ))
    }
}

fn executable(path: &Path, body: &str) -> io::Result<()> {
    fs::write(path, body)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}
