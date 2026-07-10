#[path = "support/keys.rs"]
mod keys;
#[path = "support/pty.rs"]
mod pty;
#[path = "support/screen.rs"]
mod screen;

use std::io::{self, BufRead, Read, Write};
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::time::Duration;

use keys::Key;
use pty::{PtyCommand, PtyEvent, PtySession};
use screen::{CellStyle, ScreenColor};

const DEADLINE: Duration = Duration::from_secs(5);

fn probe(mode: &str) -> PtyCommand {
    let executable = std::env::current_exe().unwrap();
    PtyCommand::new(executable, std::env::current_dir().unwrap())
        .arg("--exact")
        .arg("terminal_probe_child")
        .arg("--ignored")
        .arg("--nocapture")
        .env("JTV_TERMINAL_PROBE", mode)
        .env("TERM", "xterm-256color")
        .viewport(80, 24)
}

#[test]
fn normal_exit_and_explicit_argv_cwd_env() {
    let mut session =
        PtySession::spawn(probe("echo").env("JTV_PROBE_VALUE", "configured")).unwrap();
    #[cfg(unix)]
    assert!(session.process_group_leader().is_some());
    session
        .wait_for_screen("probe ready", DEADLINE, |frame| {
            frame.contains("probe-ready:configured")
        })
        .unwrap();
    session.send_text("hello π").unwrap();
    session.send_key(Key::Enter).unwrap();
    let frame = session
        .wait_for_screen("echoed Unicode", DEADLINE, |frame| {
            frame.contains("echo:hello π")
        })
        .unwrap();
    assert_eq!((frame.columns, frame.rows), (80, 24));
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());
    assert!(session.events().iter().any(|event| matches!(
        event,
        PtyEvent::Condition { description } if description == "echoed Unicode"
    )));
}

#[test]
fn named_keys_are_delivered() {
    let mut session = PtySession::spawn(probe("keys")).unwrap();
    session
        .wait_for_screen("keys ready", DEADLINE, |frame| frame.contains("keys-ready"))
        .unwrap();
    for key in [Key::Tab, Key::Escape, Key::Up, Key::Down, Key::Enter] {
        session.send_key(key).unwrap();
    }
    session
        .wait_for_screen("key bytes observed", DEADLINE, |frame| {
            frame.contains("bytes:09-1b-1b-5b-41-1b-5b-42-0a")
        })
        .unwrap();
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());
}

#[test]
fn resize_updates_kernel_and_virtual_screen() {
    let mut session = PtySession::spawn(probe("resize")).unwrap();
    session
        .wait_for_screen("resize ready", DEADLINE, |frame| {
            frame.contains("resize-ready")
        })
        .unwrap();
    session.resize(100, 31).unwrap();
    let frame = session.frame();
    assert_eq!((frame.columns, frame.rows), (100, 31));
    session.send_key(Key::Enter).unwrap();
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());
}

#[test]
fn ansi_styles_are_reconstructed_without_bleeding_across_resets() {
    let mut session = PtySession::spawn(probe("styles")).unwrap();
    let frame = session
        .wait_for_screen("styled probe", DEADLINE, |frame| {
            frame.contains("indexed rgb modifiers reset 界e\u{301}")
        })
        .unwrap();

    let indexed = frame.style_at_text("indexed").unwrap();
    assert_eq!(indexed.foreground, ScreenColor::Indexed(203));
    assert_eq!(indexed.background, ScreenColor::Indexed(17));
    assert!(indexed.bold);

    let rgb = frame.style_at_text("rgb").unwrap();
    assert_eq!(rgb.foreground, ScreenColor::Rgb(12, 34, 56));
    assert_eq!(rgb.background, ScreenColor::Rgb(78, 90, 123));

    let modifiers = frame.style_at_text("modifiers").unwrap();
    assert!(modifiers.dim);
    assert!(modifiers.italic);
    assert!(modifiers.underline);
    assert!(modifiers.inverse);

    let reset = frame.style_at_text("reset").unwrap();
    assert_eq!(reset, CellStyle::default());
    let reset_region = frame.find_text("reset").unwrap();
    assert!(frame.region_has_style(reset_region, CellStyle::default()));

    let wide = frame.find_text("界").unwrap();
    assert_eq!(wide.end_column - wide.start_column, 2);
    assert!(frame.cell(wide.row, wide.start_column).unwrap().wide);
    assert!(
        frame
            .cell(wide.row, wide.start_column + 1)
            .unwrap()
            .continuation
    );
    assert_eq!(
        frame.cell(wide.row, wide.end_column).unwrap().text,
        "e\u{301}"
    );

    let manifest = frame.style_manifest();
    assert!(manifest.contains("Indexed(203)"));
    assert!(manifest.contains("Rgb(12, 34, 56)"));
    assert!(manifest.contains("background: Indexed(99)"));
    assert!(!manifest.contains("\\u{1b}"));
    session.send_key(Key::Enter).unwrap();
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());
}

#[test]
fn styled_cells_survive_virtual_screen_resize() {
    let mut session = PtySession::spawn(probe("style-resize")).unwrap();
    let before = session
        .wait_for_screen("styled resize ready", DEADLINE, |frame| {
            frame.contains("styled-resize")
        })
        .unwrap();
    assert_eq!(
        before.style_at_text("styled-resize").unwrap().foreground,
        ScreenColor::Rgb(1, 2, 3)
    );
    session.resize(100, 31).unwrap();
    let after = session.frame();
    assert_eq!((after.columns, after.rows), (100, 31));
    assert_eq!(
        after.style_at_text("styled-resize").unwrap().foreground,
        ScreenColor::Rgb(1, 2, 3)
    );
    session.send_key(Key::Enter).unwrap();
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());
}

#[test]
fn alternate_screen_and_fragmented_unicode_are_reconstructed() {
    let mut session = PtySession::spawn(probe("alternate")).unwrap();
    let frame = session
        .wait_for_screen("alternate frame", DEADLINE, |frame| {
            frame.alternate_screen && frame.contains("界 fragment")
        })
        .unwrap();
    let settled = session
        .wait_for_quiet(Duration::from_millis(10), DEADLINE)
        .unwrap();
    assert!(frame.alternate_screen);
    assert_eq!(frame.text, settled.text);
    session.send_key(Key::Enter).unwrap();
    let frame = session
        .wait_for_screen("primary screen restored", DEADLINE, |frame| {
            !frame.alternate_screen && frame.contains("primary-restored")
        })
        .unwrap();
    assert!(!frame.alternate_screen);
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());
}

#[test]
fn waits_timeout_with_screen_and_event_diagnostics() {
    let mut session = PtySession::spawn(probe("wait")).unwrap();
    let error = session
        .wait_for_screen("impossible marker", Duration::from_millis(100), |frame| {
            frame.contains("never-written")
        })
        .unwrap_err();
    let diagnostic = error.to_string();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(diagnostic.contains("impossible marker"));
    assert!(diagnostic.contains("wait-ready"));
    assert!(diagnostic.contains("recent events="));
    session.terminate().unwrap();
}

#[test]
fn transcript_and_events_are_bounded_and_secret_events_are_redacted() {
    let mut session = PtySession::spawn(probe("flood")).unwrap();
    session
        .wait_for_screen("flood complete", DEADLINE, |frame| {
            frame.contains("FLOOD-DONE")
        })
        .unwrap();
    assert!(session.transcript().len() <= 256 * 1024);
    session.send_secret("do-not-log-this").unwrap();
    assert!(
        session
            .events()
            .iter()
            .any(|event| matches!(event, PtyEvent::SecretInput))
    );
    assert!(!format!("{:?}", session.events()).contains("do-not-log-this"));
    session.terminate().unwrap();
}

#[test]
fn timeout_diagnostics_redact_secret_input_even_when_the_child_echoes_it() {
    let secret = "diagnostic-secret-sentinel";
    let mut session = PtySession::spawn(probe("secret-wait")).unwrap();
    session
        .wait_for_screen("secret probe ready", DEADLINE, |frame| {
            frame.contains("secret-ready")
        })
        .unwrap();
    session.send_secret(secret).unwrap();
    session.send_key(Key::Enter).unwrap();
    let error = session
        .wait_for_screen(
            "impossible after secret",
            Duration::from_millis(100),
            |frame| frame.contains("never-written"),
        )
        .unwrap_err();
    let diagnostic = error.to_string();
    assert!(!diagnostic.contains(secret));
    assert!(diagnostic.contains("[REDACTED]"));
    session.terminate().unwrap();
}

#[test]
fn kill_and_drop_paths_reap_children() {
    let mut killed = PtySession::spawn(probe("wait")).unwrap();
    killed
        .wait_for_screen("wait ready", DEADLINE, |frame| frame.contains("wait-ready"))
        .unwrap();
    assert!(!killed.terminate().unwrap().success());

    let dropped = PtySession::spawn(probe("wait")).unwrap();
    dropped
        .wait_for_screen("wait ready", DEADLINE, |frame| frame.contains("wait-ready"))
        .unwrap();
    drop(dropped);
}

#[cfg(unix)]
#[test]
fn terminate_reaps_a_stubborn_descendant_process_group() {
    let command = PtyCommand::new("sh", std::env::current_dir().unwrap())
        .arg("-c")
        .arg("trap '' TERM; (trap '' TERM; cat) & echo descendant-ready; wait")
        .env("PATH", std::env::var_os("PATH").unwrap_or_default());
    let mut session = PtySession::spawn(command).unwrap();
    session
        .wait_for_screen("descendant ready", DEADLINE, |frame| {
            frame.contains("descendant-ready")
        })
        .unwrap();
    let group = session.process_group_leader().unwrap();
    session.terminate().unwrap();
    let group_alive = Command::new("kill")
        .args(["-0", &format!("-{group}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    assert!(!group_alive, "descendant process group {group} survived");
}

#[cfg(unix)]
#[test]
fn terminate_kills_a_stubborn_descendant_after_the_parent_exits_on_term() {
    let command = PtyCommand::new("sh", std::env::current_dir().unwrap())
        .arg("-c")
        .arg("(trap '' TERM; cat) & echo descendant-ready; wait")
        .env("PATH", std::env::var_os("PATH").unwrap_or_default());
    let mut session = PtySession::spawn(command).unwrap();
    session
        .wait_for_screen("descendant ready", DEADLINE, |frame| {
            frame.contains("descendant-ready")
        })
        .unwrap();
    let group = session.process_group_leader().unwrap();
    session.terminate().unwrap();
    let group_alive = Command::new("kill")
        .args(["-0", &format!("-{group}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    assert!(!group_alive, "descendant process group {group} survived");
}

#[cfg(unix)]
#[test]
fn drop_kills_a_stubborn_descendant_after_parent_status_was_recorded() {
    let command = PtyCommand::new("sh", std::env::current_dir().unwrap())
        .arg("-c")
        .arg("(trap '' TERM; cat) & echo descendant-ready; exit 0")
        .env("PATH", std::env::var_os("PATH").unwrap_or_default());
    let mut session = PtySession::spawn(command).unwrap();
    session
        .wait_for_screen("descendant ready", DEADLINE, |frame| {
            frame.contains("descendant-ready")
        })
        .unwrap();
    let group = session.process_group_leader().unwrap();
    assert!(session.wait_for_exit(DEADLINE).unwrap().success());
    drop(session);
    let group_alive = Command::new("kill")
        .args(["-0", &format!("-{group}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    assert!(
        !group_alive,
        "descendant process group {group} survived drop"
    );
}

#[test]
fn ctrl_c_interrupts_and_reaps_the_foreground_child() {
    let mut session = PtySession::spawn(probe("wait")).unwrap();
    session
        .wait_for_screen("wait ready", DEADLINE, |frame| frame.contains("wait-ready"))
        .unwrap();
    session.interrupt().unwrap();
    assert!(!session.wait_for_exit(DEADLINE).unwrap().success());
}

#[test]
fn key_sequences_cover_navigation_and_editing_keys() {
    assert_eq!(Key::BackTab.bytes(), b"\x1b[Z");
    assert_eq!(Key::Escape.bytes(), b"\x1b");
    assert_eq!(Key::Backspace.bytes(), b"\x7f");
    assert_eq!(Key::Delete.bytes(), b"\x1b[3~");
    assert_eq!(Key::Left.bytes(), b"\x1b[D");
    assert_eq!(Key::Right.bytes(), b"\x1b[C");
    assert_eq!(Key::Home.bytes(), b"\x1b[H");
    assert_eq!(Key::End.bytes(), b"\x1b[F");
    assert_eq!(Key::PageUp.bytes(), b"\x1b[5~");
    assert_eq!(Key::PageDown.bytes(), b"\x1b[6~");
    assert_eq!(Key::Ctrl('c').bytes(), b"\x03");
}

#[test]
#[ignore]
fn terminal_probe_child() {
    let Some(mode) = std::env::var_os("JTV_TERMINAL_PROBE") else {
        return;
    };
    let mode = mode.to_string_lossy();
    let mut input = io::stdin();
    let mut output = io::stdout();
    match mode.as_ref() {
        "echo" => {
            writeln!(
                output,
                "probe-ready:{}",
                std::env::var("JTV_PROBE_VALUE").unwrap_or_default()
            )
            .unwrap();
            output.flush().unwrap();
            let mut line = String::new();
            input.lock().read_line(&mut line).unwrap();
            println!("echo:{}", line.trim_end_matches(['\r', '\n']));
        }
        "keys" => {
            print!("keys-ready\r\n");
            output.flush().unwrap();
            let mut bytes = [0_u8; 9];
            input.read_exact(&mut bytes).unwrap();
            println!(
                "bytes:{}",
                bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join("-")
            );
        }
        "resize" => {
            print!("resize-ready\r\n");
            output.flush().unwrap();
            let mut byte = [0];
            input.read_exact(&mut byte).unwrap();
        }
        "alternate" => {
            print!("primary\x1b[?1049h\x1b[2J");
            output.flush().unwrap();
            for fragment in [b"\xe7".as_slice(), b"\x95", b"\x8c fragment"] {
                output.write_all(fragment).unwrap();
                output.flush().unwrap();
            }
            let mut byte = [0];
            input.read_exact(&mut byte).unwrap();
            print!("\x1b[?1049lprimary-restored\r\n");
        }
        "wait" => {
            print!("wait-ready\r\n");
            output.flush().unwrap();
            let mut byte = [0];
            let _ = input.read_exact(&mut byte);
        }
        "flood" => {
            for _ in 0..300_000 {
                output.write_all(b"x").unwrap();
            }
            print!("\rFLOOD-DONE");
            output.flush().unwrap();
            let mut byte = [0];
            let _ = input.read_exact(&mut byte);
        }
        "secret-wait" => {
            print!("secret-ready\r\n");
            output.flush().unwrap();
            let mut line = String::new();
            input.lock().read_line(&mut line).unwrap();
            print!("received:{}\r\n", line.trim_end_matches(['\r', '\n']));
            output.flush().unwrap();
            let mut byte = [0];
            let _ = input.read_exact(&mut byte);
        }
        "styles" => {
            // Split both CSI sequences and UTF-8 codepoints across writes to
            // exercise the same chunking behavior as a real interactive app.
            for fragment in [
                b"\x1b[38;5;2".as_slice(),
                b"03;48;5;17;1mindexed\x1b[0m ",
                b"\x1b[38;2;12;34;56;48;2;78;90;123mrgb\x1b[0m ",
                b"\x1b[2;3;4;7mmodifiers\x1b[0m reset ",
                b"\xe7",
                b"\x95",
                b"\x8c",
                b"e",
                b"\xcc",
                b"\x81 \x1b[48;5;99m  \x1b[0m\r\n",
            ] {
                output.write_all(fragment).unwrap();
                output.flush().unwrap();
            }
            let mut byte = [0];
            let _ = input.read_exact(&mut byte);
        }
        "style-resize" => {
            print!("\x1b[38;2;1;2;3mstyled-resize\x1b[0m\r\n");
            output.flush().unwrap();
            let mut byte = [0];
            let _ = input.read_exact(&mut byte);
        }
        other => panic!("unknown terminal probe mode: {other}"),
    }
}
