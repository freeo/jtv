//! Cross-platform process double for integration tests.
//!
//! This binary deliberately uses only `std`, is feature-gated, and is not part
//! of a normal `cargo build --release`.

use std::{
    env,
    fs::OpenOptions,
    io::{self, Read, Write},
    path::Path,
    process::{self, Command, Stdio},
};

fn main() {
    let executable = env::args_os()
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let name = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let code = match name {
        "just" | "jtv-test-just" => fake_just(),
        "tv" | "jtv-test-tv" => fake_tv(),
        other => fail(&format!("unknown fake-tool executable name: {other}"), 64),
    };
    process::exit(code);
}

fn fake_just() -> i32 {
    let args: Vec<String> = env::args().skip(1).collect();
    record("just", &args);
    if args == ["--version"] {
        println!("just {}", setting("JTV_TEST_JUST_VERSION", "1.53.0"));
        return setting_i32("JTV_TEST_JUST_VERSION_STATUS", 0);
    }
    if args.iter().any(|arg| arg == "--show") {
        print!(
            "{}",
            setting("JTV_TEST_JUST_SHOW", "probe:\n    @echo probe\n")
        );
        return setting_i32("JTV_TEST_JUST_SHOW_STATUS", 0);
    }
    if args.iter().any(|arg| arg == "--dump") {
        let compatibility_probe = args.iter().any(|arg| arg == "-");
        if compatibility_probe {
            let mut input = String::new();
            io::stdin().read_to_string(&mut input).unwrap();
            record("just-stdin", &[input]);
        }
        let status = setting_i32("JTV_TEST_JUST_DUMP_STATUS", 0);
        if status != 0 {
            eprintln!("{}", setting("JTV_TEST_JUST_STDERR", "fake dump failed"));
            return status;
        }
        if compatibility_probe {
            print!(r#"{{"recipes":{{"probe":{{"name":"probe","namepath":"probe"}}}}}}"#);
        } else {
            print!("{}", dump());
        }
        return 0;
    }
    setting_i32("JTV_TEST_JUST_RUN_STATUS", 0)
}

fn fake_tv() -> i32 {
    let args: Vec<String> = env::args().skip(1).collect();
    record("tv", &args);
    if args == ["--version"] {
        println!("television {}", setting("JTV_TEST_TV_VERSION", "0.15.9"));
        return setting_i32("JTV_TEST_TV_VERSION_STATUS", 0);
    }
    if args.first().is_some_and(|arg| arg == "--source-command") {
        return nested_picker();
    }
    top_level_tv()
}

fn top_level_tv() -> i32 {
    record_environment("tv-env");
    let mode = setting("JTV_TEST_TV_MODE", "cancel");
    if mode == "cancel" {
        return setting_i32("JTV_TEST_TV_CANCEL_STATUS", 0);
    }
    let source = callback(&["__tv-source"], None);
    record("source-output", std::slice::from_ref(&source.1));
    if source.0 != 0 {
        return source.0;
    }
    let rows: Vec<&str> = source.1.lines().collect();
    let ids: Vec<&str> = rows
        .iter()
        .filter_map(|row| row.split('\t').next())
        .collect();
    if let Some(id) = ids.first() {
        let preview = callback(&["__tv-preview", id], None);
        record("preview-output", &[preview.1]);
        if preview.0 != 0 {
            return preview.0;
        }
    }
    let selected: Vec<String> = match mode.as_str() {
        "one" | "one-run" => ids.iter().take(1).map(|id| (*id).to_owned()).collect(),
        "many" | "many-run" => ids.iter().rev().map(|id| (*id).to_owned()).collect(),
        "duplicate" => ids
            .first()
            .map(|id| vec![(*id).to_owned(), (*id).to_owned()])
            .unwrap_or_default(),
        "unknown" => vec!["jtv-ffffffff".into()],
        "malformed" => vec!["not-an-opaque-id".into()],
        "empty-action" => Vec::new(),
        other => return fail(&format!("unknown JTV_TEST_TV_MODE: {other}"), 64),
    };
    let mut callback_args = vec!["__tv-run".to_owned()];
    callback_args.extend(selected);
    if matches!(mode.as_str(), "empty-action" | "unknown" | "malformed") {
        let refs: Vec<&str> = callback_args.iter().map(String::as_str).collect();
        return callback(&refs, None).0;
    }
    if matches!(mode.as_str(), "one-run" | "many-run") {
        let refs: Vec<&str> = callback_args.iter().map(String::as_str).collect();
        return callback_inherit(&refs);
    }
    record("action-callback", &callback_args);
    setting_i32("JTV_TEST_TV_ACTION_STATUS", 0)
}

fn callback_inherit(args: &[&str]) -> i32 {
    let binary = env::var_os("JTV_TEST_JTV_BIN").unwrap_or_else(|| fail_exit("JTV_TEST_JTV_BIN"));
    let status = Command::new(binary)
        .args(args)
        .status()
        .unwrap_or_else(|error| {
            eprintln!("failed to spawn interactive jtv callback: {error}");
            process::exit(70);
        });
    record(
        "callback",
        &args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>(),
    );
    status.code().unwrap_or(130)
}

fn nested_picker() -> i32 {
    record_environment("picker-env");
    let source = callback(&["__picker-source"], None);
    record("picker-source-output", std::slice::from_ref(&source.1));
    if source.0 != 0 {
        return source.0;
    }
    let id = source
        .1
        .lines()
        .next()
        .and_then(|row| row.split('\t').next())
        .unwrap_or("");
    match setting("JTV_TEST_PICKER_MODE", "select").as_str() {
        "select" => println!("{id}"),
        "cancel" => {}
        "unknown" => println!("pick-ffffffff"),
        "malformed" => println!("display text; $(not executable)"),
        other => return fail(&format!("unknown JTV_TEST_PICKER_MODE: {other}"), 64),
    }
    setting_i32("JTV_TEST_PICKER_STATUS", 0)
}

fn callback(args: &[&str], stdin: Option<&[u8]>) -> (i32, String) {
    let binary = env::var_os("JTV_TEST_JTV_BIN").unwrap_or_else(|| fail_exit("JTV_TEST_JTV_BIN"));
    let mut command = Command::new(binary);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().unwrap_or_else(|error| {
        eprintln!("failed to spawn jtv callback: {error}");
        process::exit(70);
    });
    if let Some(bytes) = stdin {
        child.stdin.as_mut().unwrap().write_all(bytes).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    record(
        "callback",
        &args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>(),
    );
    if !stderr.is_empty() {
        record("callback-stderr", &[stderr]);
    }
    (output.status.code().unwrap_or(130), stdout)
}

fn dump() -> String {
    if let Ok(value) = env::var("JTV_TEST_JUST_DUMP") {
        return value;
    }
    match setting("JTV_TEST_JUST_DUMP_MODE", "valid").as_str() {
        "valid" => r#"{"recipes":{"alpha":{"name":"alpha","namepath":"alpha","doc":"alpha docs","body":["echo alpha"]},"beta":{"name":"beta","namepath":"beta","doc":"beta docs","body":["echo beta"]}},"extra_future_field":{"safe":true}}"#.into(),
        "choice" => r#"{"recipes":{"choose":{"name":"choose","namepath":"choose","parameters":[{"name":"target","kind":"singular"}],"body":["echo choose"]}}}"#.into(),
        "malformed" => "{not-json".into(),
        "missing-probe" => r#"{"recipes":{"other":{"name":"other","namepath":"other"}}}"#.into(),
        other => fail_exit(format!("JTV_TEST_JUST_DUMP_MODE={other}")),
    }
}

fn record_environment(event: &str) {
    let cwd = env::current_dir().unwrap_or_default().display().to_string();
    let values = [
        cwd,
        env::var("JTV_SESSION").unwrap_or_default(),
        env::var("JTV_PICKER_STATE").unwrap_or_default(),
        env::var("JTV_BIN").unwrap_or_default(),
    ];
    record(event, &values);
}

fn record(event: &str, fields: &[String]) {
    let Some(path) = env::var_os("JTV_TEST_RECORD") else {
        return;
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    write!(file, "{}", escape(event)).unwrap();
    for field in fields {
        write!(file, "\t{}", escape(field)).unwrap();
    }
    writeln!(file).unwrap();
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn setting(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn setting_i32(name: &str, default: i32) -> i32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn fail(message: &str, status: i32) -> i32 {
    eprintln!("{message}");
    status
}

fn fail_exit(name: impl AsRef<Path>) -> ! {
    eprintln!("missing or invalid setting: {}", name.as_ref().display());
    process::exit(64)
}
