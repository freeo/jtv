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
#[cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt};

use fake_tools::FakeTools;
use keys::Key;
use pty::{PtyCommand, PtySession};

const DEADLINE: Duration = Duration::from_secs(10);

#[cfg(unix)]
fn private_sink(tools: &FakeTools) -> std::path::PathBuf {
    let sink = tools.sandbox.root().join("jtv-history.test-session");
    fs::write(&sink, []).unwrap();
    fs::set_permissions(&sink, fs::Permissions::from_mode(0o600)).unwrap();
    sink
}

#[cfg(unix)]
fn fake_atuin(tools: &FakeTools) -> (std::path::PathBuf, std::path::PathBuf) {
    let binary = tools.sandbox.root().join("atuin");
    let log = tools.sandbox.root().join("atuin.log");
    fs::write(
        &binary,
        format!(
            "#!/bin/sh\nprintf 'cwd=%s' \"$PWD\" >> '{}'\nfor arg in \"$@\"; do printf '\\t%s' \"$arg\" >> '{}'; done\nprintf '\\n' >> '{}'\nif [ \"${{2:-}}\" = start ]; then printf 'history-id-1\\n'; fi\n",
            log.display(),
            log.display(),
            log.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    (binary, log)
}

#[test]
#[cfg(unix)]
fn integrated_action_writes_native_history_and_real_atuin_lifecycle() {
    let tools = FakeTools::new();
    tools.init();
    let sink = private_sink(&tools);
    let (atuin, atuin_log) = fake_atuin(&tools);
    let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
        .env("JTV_JUST", tools.just())
        .env("JTV_TV", tools.tv())
        .env("JTV_TEST_JTV_BIN", tools.jtv())
        .env("JTV_TEST_RECORD", &tools.record)
        .env("JTV_TEST_TV_MODE", "one-run")
        .env("JTV_TEST_JUST_RUN_STATUS", "23")
        .env("JTV_SHELL_INTEGRATION", "zsh")
        .env("JTV_HISTORY_PROTOCOL", "1")
        .env("JTV_HISTORY_SESSION", "jtv-history.test-session")
        .env("JTV_HISTORY_SINK", &sink)
        .env("JTV_ATUIN_BIN", &atuin)
        .env("ATUIN_SESSION", "jtv-test-session");
    for (key, value) in tools.sandbox.environment() {
        command = command.env(key, value);
    }
    let mut session = PtySession::spawn(command).unwrap();
    session
        .wait_for_screen("history action confirmation", DEADLINE, |frame| {
            frame.contains("Run selected recipe(s)?")
        })
        .unwrap();
    session.send_key(Key::Enter).unwrap();
    assert_eq!(session.wait_for_exit(DEADLINE).unwrap().exit_code(), 23);

    let history = fs::read_to_string(sink).unwrap();
    assert!(history.ends_with(" alpha\n"), "history={history:?}");
    let atuin = fs::read_to_string(atuin_log).unwrap();
    assert!(atuin.contains("\thistory\tstart\t--\t"), "atuin={atuin:?}");
    assert!(atuin.contains(" alpha"), "atuin={atuin:?}");
    assert!(
        atuin.contains("\thistory\tend\t--exit\t23\t--duration\t"),
        "atuin={atuin:?}"
    );
    assert!(atuin.contains("\t--\thistory-id-1"), "atuin={atuin:?}");
}

#[test]
#[cfg(unix)]
fn configured_secret_is_silently_absent_from_native_and_atuin_history() {
    const SECRET: &str = "jtv-history-secret-sentinel-4815";
    let tools = FakeTools::new();
    tools
        .sandbox
        .write_project_file(
            ".jtv.toml",
            b"[recipes.choose.params.target]\ntype='secret'\n",
        )
        .unwrap();
    tools.init();
    let sink = private_sink(&tools);
    let (atuin, atuin_log) = fake_atuin(&tools);
    let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
        .env("JTV_JUST", tools.just())
        .env("JTV_TV", tools.tv())
        .env("JTV_TEST_JTV_BIN", tools.jtv())
        .env("JTV_TEST_RECORD", &tools.record)
        .env("JTV_TEST_TV_MODE", "one-run")
        .env("JTV_TEST_JUST_DUMP_MODE", "choice")
        .env("JTV_SHELL_INTEGRATION", "zsh")
        .env("JTV_HISTORY_PROTOCOL", "1")
        .env("JTV_HISTORY_SESSION", "jtv-history.test-session")
        .env("JTV_HISTORY_SINK", &sink)
        .env("JTV_ATUIN_BIN", &atuin)
        .env("ATUIN_SESSION", "jtv-test-session");
    for (key, value) in tools.sandbox.environment() {
        command = command.env(key, value);
    }
    let mut session = PtySession::spawn(command).unwrap();
    session
        .wait_for_screen("configured secret prompt", DEADLINE, |frame| {
            frame.contains("target")
        })
        .unwrap();
    session.send_secret(SECRET).unwrap();
    session.send_key(Key::Enter).unwrap();
    session
        .wait_for_screen("redacted secret confirmation", DEADLINE, |frame| {
            frame.contains("[REDACTED]") && frame.contains("Run selected recipe(s)?")
        })
        .unwrap();
    session.send_key(Key::Enter).unwrap();
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());

    assert_eq!(fs::read(&sink).unwrap(), b"");
    assert!(!atuin_log.exists() || fs::read(&atuin_log).unwrap().is_empty());
    assert!(!String::from_utf8_lossy(&session.transcript()).contains(SECRET));
    assert!(tools.records().contains("just\tchoose\t"));
}

#[test]
#[cfg(unix)]
fn integrated_queue_records_actual_deterministic_order_and_failure_status() {
    let tools = FakeTools::new();
    tools.init();
    let sink = private_sink(&tools);
    let (atuin, atuin_log) = fake_atuin(&tools);
    let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
        .env("JTV_JUST", tools.just())
        .env("JTV_TV", tools.tv())
        .env("JTV_TEST_JTV_BIN", tools.jtv())
        .env("JTV_TEST_RECORD", &tools.record)
        .env("JTV_TEST_TV_MODE", "many-run")
        .env("JTV_TEST_JUST_FAIL_RECIPE", "beta")
        .env("JTV_TEST_JUST_FAIL_STATUS", "23")
        .env("JTV_SHELL_INTEGRATION", "zsh")
        .env("JTV_HISTORY_PROTOCOL", "1")
        .env("JTV_HISTORY_SESSION", "jtv-history.test-session")
        .env("JTV_HISTORY_SINK", &sink)
        .env("JTV_ATUIN_BIN", &atuin)
        .env("ATUIN_SESSION", "jtv-test-session");
    for (key, value) in tools.sandbox.environment() {
        command = command.env(key, value);
    }
    let mut session = PtySession::spawn(command).unwrap();
    session
        .wait_for_screen("queue history confirmation", DEADLINE, |frame| {
            frame.contains("Run selected recipe(s)?")
        })
        .unwrap();
    session.send_key(Key::Enter).unwrap();
    assert_eq!(session.wait_for_exit(DEADLINE).unwrap().exit_code(), 23);

    let history = fs::read_to_string(sink).unwrap();
    let lines = history.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2, "history={history:?}");
    assert!(lines[0].ends_with(" alpha"), "history={history:?}");
    assert!(lines[1].ends_with(" beta"), "history={history:?}");
    let atuin = fs::read_to_string(atuin_log).unwrap();
    let end_calls = atuin
        .lines()
        .filter(|line| line.contains("\thistory\tend\t"))
        .collect::<Vec<_>>();
    assert_eq!(end_calls.len(), 2, "atuin={atuin:?}");
    assert!(end_calls[0].contains("\t--exit\t0\t"));
    assert!(end_calls[1].contains("\t--exit\t23\t"));
}

#[test]
#[cfg(unix)]
fn cancellation_and_decline_add_no_synthetic_history() {
    for mode in ["cancel", "one-run"] {
        let tools = FakeTools::new();
        tools.init();
        let sink = private_sink(&tools);
        let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
            .env("JTV_JUST", tools.just())
            .env("JTV_TV", tools.tv())
            .env("JTV_TEST_JTV_BIN", tools.jtv())
            .env("JTV_TEST_RECORD", &tools.record)
            .env("JTV_TEST_TV_MODE", mode)
            .env("JTV_SHELL_INTEGRATION", "zsh")
            .env("JTV_HISTORY_PROTOCOL", "1")
            .env("JTV_HISTORY_SESSION", "jtv-history.test-session")
            .env("JTV_HISTORY_SINK", &sink);
        for (key, value) in tools.sandbox.environment() {
            command = command.env(key, value);
        }
        let mut session = PtySession::spawn(command).unwrap();
        if mode == "one-run" {
            session
                .wait_for_screen("history decline confirmation", DEADLINE, |frame| {
                    frame.contains("Run selected recipe(s)?")
                })
                .unwrap();
            session.send_text("n").unwrap();
            session.send_key(Key::Enter).unwrap();
        }
        assert!(session.wait_for_exit(DEADLINE).unwrap().success());
        assert_eq!(fs::read(&sink).unwrap(), b"", "mode={mode}");
    }
}

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

#[test]
fn tab_completion_forwards_partial_query_and_replaces_the_string_buffer() {
    let tools = FakeTools::new();
    let selected = "docs/guide λ; $draft.md";
    tools
        .sandbox
        .write_project_file(selected, b"guide\n")
        .unwrap();
    tools.init();
    let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
        .env("JTV_JUST", tools.just())
        .env("JTV_TV", tools.tv())
        .env("JTV_TEST_JTV_BIN", tools.jtv())
        .env("JTV_TEST_RECORD", &tools.record)
        .env("JTV_TEST_TV_MODE", "one-run")
        .env("JTV_TEST_JUST_DUMP_MODE", "choice")
        .env("JTV_TEST_PICKER_SELECT_DISPLAY", selected);
    for (key, value) in tools.sandbox.environment() {
        command = command.env(key, value);
    }
    let mut session = PtySession::spawn(command).unwrap();
    session
        .wait_for_screen("ordinary string prompt", DEADLINE, |frame| {
            frame.contains("target")
        })
        .unwrap();
    session.send_text("docs").unwrap();
    session.send_key(Key::Tab).unwrap();
    session
        .wait_for_screen("completed string prompt", DEADLINE, |frame| {
            frame.contains("docs/guide λ; $draft.md")
        })
        .unwrap();
    session.send_key(Key::Enter).unwrap();
    session
        .wait_for_screen("confirmation after TAB completion", DEADLINE, |frame| {
            frame.contains("Run selected recipe(s)?")
        })
        .unwrap();
    session.send_key(Key::Enter).unwrap();
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());

    let records = tools.records();
    assert!(records.contains("picker-argv"));
    assert!(records.contains("\t--input\tdocs"), "records={records}");
    assert!(
        records.contains("just\tchoose\tdocs/guide λ; $draft.md"),
        "records={records}"
    );
    assert!(records.contains("picker-source-output"));
}

#[test]
fn cancelling_tab_completion_preserves_partial_text_for_manual_editing() {
    let tools = FakeTools::new();
    tools
        .sandbox
        .write_project_file("docs/guide.md", b"guide\n")
        .unwrap();
    tools.init();
    let mut command = PtyCommand::new(tools.jtv(), tools.sandbox.project())
        .env("JTV_JUST", tools.just())
        .env("JTV_TV", tools.tv())
        .env("JTV_TEST_JTV_BIN", tools.jtv())
        .env("JTV_TEST_RECORD", &tools.record)
        .env("JTV_TEST_TV_MODE", "one-run")
        .env("JTV_TEST_JUST_DUMP_MODE", "choice")
        .env("JTV_TEST_PICKER_MODE", "cancel");
    for (key, value) in tools.sandbox.environment() {
        command = command.env(key, value);
    }
    let mut session = PtySession::spawn(command).unwrap();
    session
        .wait_for_screen("ordinary string prompt", DEADLINE, |frame| {
            frame.contains("target")
        })
        .unwrap();
    session.send_text("docs").unwrap();
    session.send_key(Key::Tab).unwrap();
    session
        .wait_for_screen(
            "restored prompt after nested cancellation",
            DEADLINE,
            |frame| frame.contains("docs"),
        )
        .unwrap();
    session.send_text("-manual").unwrap();
    session.send_key(Key::Enter).unwrap();
    session
        .wait_for_screen("confirmation after manual completion", DEADLINE, |frame| {
            frame.contains("Run selected recipe(s)?")
        })
        .unwrap();
    session.send_key(Key::Enter).unwrap();
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());

    let records = tools.records();
    assert!(records.contains("\t--input\tdocs"), "records={records}");
    assert!(
        records.contains("just\tchoose\tdocs-manual"),
        "records={records}"
    );
}
