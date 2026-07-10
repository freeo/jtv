use std::{path::Path, process::Command};

#[test]
#[ignore = "builds a fresh release tree; run via `just verify-release-artifacts`"]
fn default_release_excludes_test_helper() {
    let target = tempfile::tempdir().expect("temporary target directory");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "build",
            "--release",
            "--locked",
            "--no-default-features",
            "--target-dir",
        ])
        .arg(target.path())
        .output()
        .expect("run clean release build");
    assert!(
        output.status.success(),
        "release build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let release = target.path().join("release");
    assert!(executable(&release, "jtv").is_file());
    assert!(
        !executable(&release, "jtv-test-tool").exists(),
        "test helper leaked into the default release"
    );
}

fn executable(directory: &Path, name: &str) -> std::path::PathBuf {
    directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}
