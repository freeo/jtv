#[path = "support/artifacts.rs"]
mod artifacts;
#[path = "support/sandbox.rs"]
mod sandbox;

use artifacts::{FailureArtifacts, Sanitizer, SemanticEvent};
use sandbox::TestSandbox;
use std::fs;

#[test]
fn sandbox_isolates_all_writable_process_state() {
    let sandbox = TestSandbox::new().unwrap();
    let environment = sandbox.environment();
    for key in [
        "HOME",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
        "TELEVISION_CONFIG",
        "JTV_TV_CABLE_DIR",
        "JTV_TEST_ARTIFACT_DIR",
    ] {
        let path = std::path::Path::new(environment.get(std::ffi::OsStr::new(key)).unwrap());
        assert!(
            path.starts_with(sandbox.root()),
            "{key} escaped sandbox: {path:?}"
        );
        assert!(path.is_dir());
    }
    let fixture = sandbox
        .write_project_file("nested/value.txt", b"fixture")
        .unwrap();
    assert_eq!(fs::read_to_string(fixture).unwrap(), "fixture");
    assert!(
        sandbox
            .command("definitely-not-run")
            .get_current_dir()
            .unwrap()
            .starts_with(sandbox.root())
    );
    assert!(
        sandbox.home().is_dir()
            && sandbox.runtime().is_dir()
            && sandbox.television().is_dir()
            && sandbox.cable().is_dir()
            && sandbox.artifacts().is_dir()
    );
}

#[test]
fn fixture_paths_cannot_escape_project() {
    let sandbox = TestSandbox::new().unwrap();
    assert!(sandbox.write_project_file("../escaped", b"no").is_err());
}

#[test]
fn sanitizer_redacts_secrets_and_normalizes_paths() {
    let sandbox = TestSandbox::new().unwrap();
    let secret = "JTV_SECRET_SENTINEL_931";
    let mut sanitizer = Sanitizer::new(sandbox.root());
    sanitizer.add_secret(secret);
    let clean = sanitizer.sanitize(&format!(
        "path={}\\child token={secret}",
        sandbox.root().display()
    ));
    assert_eq!(clean, "path=<SANDBOX>/child token=<SECRET>");
    assert!(!sanitizer.contains_secret(clean.as_bytes()));
}

#[test]
fn sanitizer_normalizes_workspace_paths_separately_from_the_sandbox() {
    let root = tempfile::tempdir().unwrap();
    let sandbox = root.path().join("sandbox");
    let workspace = root.path().join("workspace");
    let mut sanitizer = Sanitizer::new(&sandbox);
    sanitizer.add_workspace_root(&workspace);
    let clean = sanitizer.sanitize(&format!(
        "binary={} input={}",
        workspace.join("target/debug/jtv").display(),
        sandbox.join("project/justfile").display()
    ));
    assert_eq!(
        clean,
        "binary=<WORKSPACE>/target/debug/jtv input=<SANDBOX>/project/justfile"
    );
}

#[test]
fn artifacts_are_opt_in_bounded_and_secret_safe() {
    let sandbox = TestSandbox::new().unwrap();
    let workspace = sandbox.root().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let secret = "JTV_SECRET_SENTINEL_932";
    let mut artifacts =
        FailureArtifacts::new("secret_case", &workspace, sandbox.root(), true).unwrap();
    artifacts.sanitizer_mut().add_secret(secret);
    artifacts.push_transcript(format!("{}\n{secret}\n", "x".repeat(300_000)).as_bytes());
    artifacts.push_event(SemanticEvent::Spawn {
        program: sandbox.root().join("bin/jtv").display().to_string(),
        args: vec![secret.into()],
    });
    artifacts.push_event(SemanticEvent::SecretInput);
    artifacts.push_event(SemanticEvent::Key {
        name: "Enter".into(),
    });
    artifacts.push_event(SemanticEvent::Text {
        value: "public".into(),
    });
    artifacts.push_event(SemanticEvent::Resize {
        columns: 120,
        rows: 40,
    });
    artifacts.push_event(SemanticEvent::ScreenCondition {
        description: "browser visible".into(),
    });
    artifacts.push_event(SemanticEvent::Exit { status: Some(7) });
    artifacts.push_event(SemanticEvent::Note {
        message: "diagnostic".into(),
    });
    let directory = artifacts
        .persist(
            &format!("screen {secret}"),
            &format!("cwd={}", sandbox.project().display()),
        )
        .unwrap()
        .unwrap();
    assert!(directory.starts_with(workspace.join("target/jtv-test-artifacts")));
    for entry in fs::read_dir(&directory).unwrap() {
        let bytes = fs::read(entry.unwrap().path()).unwrap();
        assert!(bytes.len() <= 256 * 1024);
        assert!(
            !bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes())
        );
        assert!(
            !String::from_utf8_lossy(&bytes).contains(sandbox.root().to_string_lossy().as_ref())
        );
    }
    assert!(
        fs::read_to_string(directory.join("events.tsv"))
            .unwrap()
            .contains("secret-input\t<REDACTED>")
    );
}

#[test]
fn disabled_artifacts_write_nothing() {
    let sandbox = TestSandbox::new().unwrap();
    let workspace = sandbox.root().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let mut artifacts =
        FailureArtifacts::new("no_failure", &workspace, sandbox.root(), false).unwrap();
    assert_eq!(artifacts.persist("screen", "metadata").unwrap(), None);
    assert!(!workspace.join("target/jtv-test-artifacts").exists());
}

#[test]
fn dropping_sandbox_removes_temporary_state() {
    let root = {
        let sandbox = TestSandbox::new().unwrap();
        let root = sandbox.root().to_owned();
        sandbox.write_project_file("state", b"temporary").unwrap();
        root
    };
    assert!(!root.exists());
}
