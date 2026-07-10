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
use keys::Key;
use pty::{PtyCommand, PtySession};

const DEADLINE: Duration = Duration::from_secs(10);

#[test]
fn successful_fake_tv_actions_execute_through_jtv_in_a_real_pty() {
    for (mode, expected) in [
        ("one-run", vec!["just\talpha"]),
        ("many-run", vec!["just\talpha", "just\tbeta"]),
    ] {
        let tools = FakeTools::new();
        tools.init();
        let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
            .env("JTV_JUST", tools.just())
            .env("JTV_TV", tools.tv())
            .env("JTV_TEST_JTV_BIN", tools.jtv())
            .env("JTV_TEST_RECORD", &tools.record)
            .env("JTV_TEST_TV_MODE", mode);
        for (key, value) in tools.sandbox.environment() {
            command = command.env(key, value);
        }
        let mut session = PtySession::spawn(command).unwrap();
        session
            .wait_for_screen("interactive action confirmation", DEADLINE, |frame| {
                frame.contains("Run selected recipe(s)?")
            })
            .unwrap();
        session.send_key(Key::Enter).unwrap();
        assert!(session.wait_for_exit(DEADLINE).unwrap().success());
        let records = tools.records();
        for event in expected {
            assert!(
                records.lines().any(|line| line == event),
                "missing {event:?} in fake action records"
            );
        }
        assert!(records.contains("callback\t__tv-run"));
        assert!(!records.contains("display text;"));
    }
}

#[test]
fn fake_tv_action_failure_propagates_through_the_interactive_callback() {
    let tools = FakeTools::new();
    tools.init();
    let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
        .env("JTV_JUST", tools.just())
        .env("JTV_TV", tools.tv())
        .env("JTV_TEST_JTV_BIN", tools.jtv())
        .env("JTV_TEST_RECORD", &tools.record)
        .env("JTV_TEST_TV_MODE", "one-run")
        .env("JTV_TEST_JUST_RUN_STATUS", "23");
    for (key, value) in tools.sandbox.environment() {
        command = command.env(key, value);
    }
    let mut session = PtySession::spawn(command).unwrap();
    session
        .wait_for_screen("interactive action confirmation", DEADLINE, |frame| {
            frame.contains("Run selected recipe(s)?")
        })
        .unwrap();
    session.send_key(Key::Enter).unwrap();
    assert_eq!(session.wait_for_exit(DEADLINE).unwrap().exit_code(), 23);
}

#[test]
fn nested_picker_results_are_consumed_through_the_real_tv_picker_boundary() {
    for picker_mode in ["select", "cancel", "unknown", "malformed"] {
        let tools = FakeTools::new();
        tools
            .sandbox
            .write_project_file(
                ".jtv.toml",
                b"[recipes.choose.params.target]\ntype='choice'\nvalues=['dev','prod']\n",
            )
            .unwrap();
        tools.init();
        let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
            .env("JTV_JUST", tools.just())
            .env("JTV_TV", tools.tv())
            .env("JTV_TEST_JTV_BIN", tools.jtv())
            .env("JTV_TEST_RECORD", &tools.record)
            .env("JTV_TEST_TV_MODE", "one-run")
            .env("JTV_TEST_JUST_DUMP_MODE", "choice")
            .env("JTV_TEST_PICKER_MODE", picker_mode);
        for (key, value) in tools.sandbox.environment() {
            command = command.env(key, value);
        }
        let mut session = PtySession::spawn(command).unwrap();
        if picker_mode == "select" {
            session
                .wait_for_screen("confirmation after nested choice", DEADLINE, |frame| {
                    frame.contains("Run selected recipe(s)?")
                })
                .unwrap();
            session.send_key(Key::Enter).unwrap();
            assert!(session.wait_for_exit(DEADLINE).unwrap().success());
            assert!(
                tools
                    .records()
                    .lines()
                    .any(|line| line == "just\tchoose\tdev")
            );
        } else {
            let expected = if picker_mode == "cancel" { 130 } else { 1 };
            assert_eq!(
                session.wait_for_exit(DEADLINE).unwrap().exit_code(),
                expected
            );
            assert!(
                !tools
                    .records()
                    .lines()
                    .any(|line| line == "just\tchoose\tdev")
            );
        }
        assert!(tools.records().contains("picker-source-output"));
    }
}
