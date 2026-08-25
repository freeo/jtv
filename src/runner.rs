//! Process execution and queue policy.

use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use crate::{Error, Result, command::CommandPlan};

pub trait Executor {
    fn execute(&mut self, plan: &CommandPlan) -> Result<i32>;
}

/// The result of an executor call as observed at the real process boundary.
#[derive(Clone, Copy, Debug)]
pub enum AttemptOutcome<'a> {
    Exit(i32),
    Error(&'a Error),
}

/// Best-effort execution lifecycle hooks. Observer work must handle its own
/// failures; hooks cannot alter command execution or its exit status.
pub trait ExecutionObserver {
    fn before(&mut self, _plan: &CommandPlan) {}
    fn after(&mut self, _plan: &CommandPlan, _outcome: AttemptOutcome<'_>, _duration: Duration) {}
}

#[derive(Default)]
struct NoopObserver;

impl ExecutionObserver for NoopObserver {}

#[derive(Default)]
pub struct ProcessExecutor;

impl Executor for ProcessExecutor {
    fn execute(&mut self, plan: &CommandPlan) -> Result<i32> {
        let status = Command::new(&plan.program)
            .args(&plan.args)
            .current_dir(&plan.cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|source| Error::Spawn {
                program: plan.program.display().to_string(),
                source,
            })?;
        Ok(exit_code(status))
    }
}

pub fn run_queue<E: Executor>(plans: &[CommandPlan], executor: &mut E) -> Result<i32> {
    run_queue_observed(plans, executor, &mut NoopObserver)
}

pub fn run_queue_observed<E: Executor, O: ExecutionObserver>(
    plans: &[CommandPlan],
    executor: &mut E,
    observer: &mut O,
) -> Result<i32> {
    for plan in plans {
        observer.before(plan);
        let started = Instant::now();
        let result = executor.execute(plan);
        let duration = started.elapsed();
        let status = match result {
            Ok(status) => {
                observer.after(plan, AttemptOutcome::Exit(status), duration);
                status
            }
            Err(error) => {
                observer.after(plan, AttemptOutcome::Error(&error), duration);
                return Err(error);
            }
        };
        if status != 0 {
            return Ok(status);
        }
    }
    Ok(0)
}

fn exit_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}
