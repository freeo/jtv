#![allow(dead_code)]

#[path = "support/fake_tools.rs"]
mod fake_tools;
#[path = "support/keys.rs"]
mod keys;
#[path = "support/pty.rs"]
mod pty;
#[path = "support/sandbox.rs"]
mod sandbox;
#[path = "support/screen.rs"]
mod screen;

use std::time::Duration;

use fake_tools::FakeTools;
use pty::{PtyCommand, PtySession};

const DEADLINE: Duration = Duration::from_secs(5);

fn launch(tools: &FakeTools, environment: &[(&str, &str)]) -> PtySession {
    let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
        .env("JTV_JUST", tools.just())
        .env("JTV_TV", tools.tv())
        .env("JTV_TEST_JTV_BIN", tools.jtv())
        .env("JTV_TEST_RECORD", &tools.record);
    for (key, value) in tools.sandbox.environment() {
        command = command.env(key, value);
    }
    for (key, value) in environment {
        command = command.env(key, value);
    }
    PtySession::spawn(command).unwrap()
}

fn assert_tty_failure(tools: &FakeTools, environment: &[(&str, &str)], expected: &str) {
    let mut session = launch(tools, environment);
    session
        .wait_for_screen("actionable terminal diagnostic", DEADLINE, |frame| {
            frame.contains(expected)
        })
        .unwrap();
    assert_eq!(session.wait_for_exit(DEADLINE).unwrap().exit_code(), 1);
}

#[test]
fn incompatible_just_and_television_are_actionable_in_a_tty() {
    let just = FakeTools::new();
    assert_tty_failure(
        &just,
        &[("JTV_TEST_JUST_VERSION", "1.52.0")],
        "just 1.52.0 is unsupported",
    );

    let tv = FakeTools::new();
    assert_tty_failure(
        &tv,
        &[("JTV_TEST_TV_VERSION", "0.15.8")],
        "television 0.15.8 is unsupported",
    );
}

#[test]
fn missing_just_and_television_are_actionable_in_a_tty() {
    let just = FakeTools::new();
    let missing_just = just.sandbox.root().join("missing-just");
    let missing_just = missing_just.to_str().unwrap();
    assert_tty_failure(
        &just,
        &[("JTV_JUST", missing_just)],
        &format!("failed to run {missing_just}"),
    );

    let tv = FakeTools::new();
    let missing_tv = tv.sandbox.root().join("missing-tv");
    let missing_tv = missing_tv.to_str().unwrap();
    assert_tty_failure(
        &tv,
        &[("JTV_TV", missing_tv)],
        &format!("failed to run {missing_tv}"),
    );
}

#[test]
fn malformed_just_json_is_actionable_in_a_tty() {
    let tools = FakeTools::new();
    assert_tty_failure(
        &tools,
        &[("JTV_TEST_JUST_DUMP_MODE", "malformed")],
        "unable to parse Justfile JSON",
    );
}

#[test]
fn modified_channel_is_actionable_in_a_tty() {
    let tools = FakeTools::new();
    tools.init();
    std::fs::write(
        tools.sandbox.cable().join("jtv-recipes.toml"),
        "user-owned channel",
    )
    .unwrap();
    assert_tty_failure(
        &tools,
        &[],
        "channel is missing or outdated; run `jtv init`",
    );
}
