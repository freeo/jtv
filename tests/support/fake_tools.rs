use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use assert_cmd::cargo::cargo_bin;

use crate::sandbox::TestSandbox;

pub struct FakeTools {
    pub sandbox: TestSandbox,
    pub record: PathBuf,
    just: PathBuf,
    tv: PathBuf,
    jtv: PathBuf,
}

impl FakeTools {
    pub fn new() -> Self {
        let sandbox = TestSandbox::new().expect("test sandbox");
        let record = sandbox.root().join("process-events.log");
        let helper = cargo_bin!("jtv-test-tool").to_path_buf();
        let suffix = std::env::consts::EXE_SUFFIX;
        let just = sandbox.root().join(format!("just{suffix}"));
        let tv = sandbox.root().join(format!("tv{suffix}"));
        copy_executable(&helper, &just);
        copy_executable(&helper, &tv);
        Self {
            sandbox,
            record,
            just,
            tv,
            jtv: cargo_bin!("jtv").to_path_buf(),
        }
    }

    pub fn init(&self) {
        self.jtv_command()
            .arg("init")
            .status()
            .expect("run init")
            .success()
            .then_some(())
            .expect("init succeeds");
    }

    pub fn jtv_command(&self) -> Command {
        let mut command = self.sandbox.command(&self.jtv);
        command
            .env("JTV_JUST", &self.just)
            .env("JTV_TV", &self.tv)
            .env("JTV_TEST_JTV_BIN", &self.jtv)
            .env("JTV_TEST_RECORD", &self.record);
        command
    }

    pub fn records(&self) -> String {
        fs::read_to_string(&self.record).unwrap_or_default()
    }

    pub fn just(&self) -> &Path {
        &self.just
    }
    pub fn tv(&self) -> &Path {
        &self.tv
    }
    pub fn jtv(&self) -> &Path {
        &self.jtv
    }
}

fn copy_executable(source: &Path, destination: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(source, destination).expect("symlink fake-tool executable");
    #[cfg(windows)]
    fs::copy(source, destination).expect("copy fake-tool executable");
}
