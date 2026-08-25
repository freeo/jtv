#[cfg(unix)]
use jtv::runner::ProcessExecutor;
use jtv::{
    Error, Result,
    command::CommandPlan,
    runner::{AttemptOutcome, ExecutionObserver, Executor, run_queue, run_queue_observed},
};
use std::{collections::VecDeque, path::PathBuf, time::Duration};

struct Fake {
    statuses: VecDeque<i32>,
    seen: Vec<String>,
}
impl Executor for Fake {
    fn execute(&mut self, plan: &CommandPlan) -> Result<i32> {
        self.seen.push(plan.args[0].to_string_lossy().into());
        Ok(self.statuses.pop_front().unwrap())
    }
}
fn plan(name: &str) -> CommandPlan {
    CommandPlan {
        program: "just".into(),
        cwd: PathBuf::from("."),
        args: vec![name.into()],
        redacted_args: vec![name.into()],
        contains_secret: false,
    }
}

#[test]
fn runs_in_order_and_stops_on_first_failure() {
    let mut fake = Fake {
        statuses: [0, 23, 0].into(),
        seen: vec![],
    };
    assert_eq!(
        run_queue(&[plan("one"), plan("two"), plan("three")], &mut fake).unwrap(),
        23
    );
    assert_eq!(fake.seen, ["one", "two"]);
}

#[test]
fn empty_and_successful_queues_return_success() {
    let mut fake = Fake {
        statuses: [0, 0].into(),
        seen: vec![],
    };
    assert_eq!(run_queue(&[], &mut fake).unwrap(), 0);
    assert_eq!(
        run_queue(&[plan("one"), plan("two")], &mut fake).unwrap(),
        0
    );
}

#[derive(Default)]
struct Observer {
    events: Vec<String>,
    durations: Vec<Duration>,
}

impl ExecutionObserver for Observer {
    fn before(&mut self, plan: &CommandPlan) {
        self.events
            .push(format!("before:{}", plan.args[0].to_string_lossy()));
    }

    fn after(&mut self, plan: &CommandPlan, outcome: AttemptOutcome<'_>, duration: Duration) {
        let outcome = match outcome {
            AttemptOutcome::Exit(status) => format!("exit:{status}"),
            AttemptOutcome::Error(error) => format!("error:{error}"),
        };
        self.events.push(format!(
            "after:{}:{outcome}",
            plan.args[0].to_string_lossy()
        ));
        self.durations.push(duration);
    }
}

#[test]
fn observer_brackets_each_real_attempt_and_respects_early_failure() {
    let mut fake = Fake {
        statuses: [0, 23, 0].into(),
        seen: vec![],
    };
    let mut observer = Observer::default();
    assert_eq!(
        run_queue_observed(
            &[plan("one"), plan("two"), plan("never")],
            &mut fake,
            &mut observer,
        )
        .unwrap(),
        23
    );
    assert_eq!(
        observer.events,
        [
            "before:one",
            "after:one:exit:0",
            "before:two",
            "after:two:exit:23"
        ]
    );
    assert_eq!(observer.durations.len(), 2);
}

struct Broken;

impl Executor for Broken {
    fn execute(&mut self, _plan: &CommandPlan) -> Result<i32> {
        Err(Error::Message("spawn exploded".into()))
    }
}

#[test]
fn observer_receives_executor_errors_before_the_error_is_returned() {
    let mut observer = Observer::default();
    let error = run_queue_observed(&[plan("broken")], &mut Broken, &mut observer).unwrap_err();
    assert_eq!(error.to_string(), "spawn exploded");
    assert_eq!(
        observer.events,
        ["before:broken", "after:broken:error:spawn exploded"]
    );
}

#[cfg(unix)]
#[test]
fn process_executor_preserves_os_arguments_without_shell_reinterpretation() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let executable = temp.path().join("record-args");
    let log = temp.path().join("args.bin");
    std::fs::write(
        &executable,
        "#!/bin/sh\nlog=$1\nshift\nprintf '%s\\0' \"$@\" > \"$log\"\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).unwrap();

    let plan = CommandPlan {
        program: executable,
        cwd: temp.path().into(),
        args: vec![
            log.clone().into_os_string(),
            "a b".into(),
            ";$(printf hacked)".into(),
            "line1\nline2".into(),
        ],
        redacted_args: vec![],
        contains_secret: false,
    };
    let status = ProcessExecutor.execute(&plan).unwrap();
    assert_eq!(status, 0);
    let values: Vec<_> = std::fs::read(log)
        .unwrap()
        .split(|byte| *byte == 0)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(
        values,
        [
            b"a b".to_vec(),
            b";$(printf hacked)".to_vec(),
            b"line1\nline2".to_vec()
        ]
    );
}
