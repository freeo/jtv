//! Process execution and queue policy.

use std::process::{Command, Stdio};

use crate::{Error, Result, command::CommandPlan};

pub trait Executor {
    fn execute(&mut self, plan: &CommandPlan) -> Result<i32>;
}

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
    for plan in plans {
        let status = executor.execute(plan)?;
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
