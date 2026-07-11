#![cfg(unix)]
#![allow(dead_code)]

#[path = "support/artifacts.rs"]
mod artifacts;
#[path = "support/keys.rs"]
mod keys;
#[path = "support/pty.rs"]
mod pty;
#[path = "support/sandbox.rs"]
mod sandbox;
#[path = "support/scenarios.rs"]
mod scenarios;
#[path = "support/screen.rs"]
mod screen;

use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use std::time::Instant;

use keys::Key;
use scenarios::RealTvScenario;

fn assert_status(status: portable_pty::ExitStatus, expected: u32) {
    assert_eq!(
        status.exit_code(),
        expected,
        "unexpected status: {status:?}"
    );
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn browse_filter_visible_preview_and_run() {
    let mut scenario = RealTvScenario::launch("browse-preview-run").unwrap();
    scenario.select_recipe("simple");
    let preview = scenario.wait(
        "visible recipe documentation",
        "Write a no-argument marker.",
    );
    assert!(
        preview.alternate_screen,
        "Television must own the alternate screen"
    );
    scenario.key(Key::Enter);
    scenario.confirm();
    assert_status(scenario.exit(), 0);
    assert_eq!(scenario.events(), ["simple"]);
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn cycles_to_faithful_definition_and_runs_the_explicit_dry_run_action() {
    let mut scenario = RealTvScenario::launch("definition-and-dry-run").unwrap();
    scenario.select_recipe("simple");
    scenario.key(Key::Ctrl('f'));
    let definition = scenario.wait("faithful just definition preview", "simple:");
    assert!(definition.contains("@printf 'simple\\n'"));

    scenario.key(Key::Ctrl('x'));
    scenario.wait("Television action menu", "Dry-run selected recipes");
    scenario.text("dry-run");
    scenario.key(Key::Enter);
    scenario.confirm();
    assert_status(scenario.exit(), 0);
    assert!(
        scenario.events().is_empty(),
        "just --dry-run must print the recipe command without executing it"
    );
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn live_resize_preserves_filter_identity_preview_and_focus() {
    let mut scenario = RealTvScenario::launch("live-resize").unwrap();
    scenario.select_recipe("capture");
    scenario.session.resize(80, 24).unwrap();
    let narrow = scenario.wait(
        "narrow resized recipe and preview",
        "Capture one argument literally.",
    );
    assert_eq!((narrow.columns, narrow.rows), (80, 24));
    assert!(narrow.contains("capture"));
    assert!(narrow.cursor_visible);

    scenario.session.resize(120, 40).unwrap();
    let wide = scenario.wait(
        "wide resized recipe and preview",
        "Capture one argument literally.",
    );
    assert_eq!((wide.columns, wide.rows), (120, 40));
    assert!(wide.contains("capture"));
    assert!(wide.cursor_visible);
    scenario.key(Key::Escape);
    assert_status(scenario.exit(), 0);
    assert!(scenario.events().is_empty());
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn television_caches_repeated_definition_previews() {
    let mut scenario = RealTvScenario::launch("definition-cache").unwrap();
    scenario.select_recipe("simple");
    scenario.key(Key::Ctrl('f'));
    scenario.wait("first Definition callback", "simple:");
    scenario.key(Key::Ctrl('f'));
    scenario.wait("return to Details", "Write a no-argument marker.");
    scenario.key(Key::Ctrl('f'));
    scenario.wait("cached Definition callback", "simple:");
    let invocations = scenario.just_invocations();
    let show_count = invocations
        .iter()
        .filter(|invocation| invocation.contains("--show simple"))
        .count();
    assert_eq!(
        show_count, 1,
        "Television must cache the identical Definition command"
    );
    assert_eq!(
        invocations
            .iter()
            .filter(|invocation| invocation.contains("--dump"))
            .count(),
        2,
        "launch performs one contract dump and one project dump, never one subprocess per row"
    );
    assert_eq!(
        invocations
            .iter()
            .filter(|invocation| invocation.as_str() == "--version")
            .count(),
        1
    );
    assert_eq!(
        invocations.len(),
        4,
        "the complete just process set is version + two dumps + one cached Definition: {invocations:?}"
    );
    scenario.key(Key::Escape);
    assert_status(scenario.exit(), 0);
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn scalar_defaults_alias_module_and_variadics() {
    let mut scalar = RealTvScenario::launch("scalar-adversarial").unwrap();
    scalar.select_recipe("capture");
    scalar.key(Key::Enter);
    scalar.wait("string prompt", "value");
    let literal = "spaces 'quoted';$(printf hacked) λ";
    scalar.text(literal);
    scalar.key(Key::Enter);
    scalar.confirm();
    let scalar_status = scalar.exit();
    assert_eq!(
        scalar_status.exit_code(),
        0,
        "scalar events={:?}",
        scalar.events()
    );
    assert_eq!(scalar.events(), [format!("capture:{literal}")]);

    let mut literal_default = RealTvScenario::launch("literal-default").unwrap();
    literal_default.select_recipe("literal-default");
    literal_default.key(Key::Enter);
    literal_default.wait("literal default prompt", "fallback");
    literal_default.key(Key::Enter);
    literal_default.confirm();
    let status = literal_default.exit();
    assert_eq!(
        status.exit_code(),
        0,
        "literal events={:?}",
        literal_default.events()
    );
    assert_eq!(literal_default.events(), ["literal-default:fallback"]);

    let mut expression = RealTvScenario::launch("expression-default").unwrap();
    expression.select_recipe("expression-default");
    expression.key(Key::Enter);
    expression.wait("expression default prompt", "value");
    expression.key(Key::Enter);
    expression.confirm();
    let status = expression.exit();
    assert_eq!(
        status.exit_code(),
        0,
        "expression events={:?}",
        expression.events()
    );
    assert_eq!(
        expression.events(),
        ["expression-default:expression-fallback"]
    );

    let mut alias = RealTvScenario::launch("alias").unwrap();
    alias.select_recipe("simple-alias");
    alias.key(Key::Enter);
    alias.confirm();
    let status = alias.exit();
    assert_eq!(status.exit_code(), 0, "alias events={:?}", alias.events());
    assert_eq!(alias.events(), ["simple"]);

    let mut module = RealTvScenario::launch("module").unwrap();
    module.wait("root source", "Root");
    module.cycle_source("Subfolders");
    module.cycle_source("Modules");
    module.select_recipe("module-run");
    module.key(Key::Enter);
    module.wait("module default prompt", "module-default");
    module.key(Key::Enter);
    module.confirm();
    let status = module.exit();
    assert_eq!(status.exit_code(), 0, "module events={:?}", module.events());
    assert_eq!(module.events(), ["module:module-default"]);

    let mut variadic = RealTvScenario::launch("variadic").unwrap();
    variadic.select_recipe("variadic");
    variadic.key(Key::Enter);
    variadic.wait("first variadic prompt", "value 1");
    variadic.text("first item");
    variadic.key(Key::Enter);
    variadic.wait("second variadic prompt", "value 2");
    variadic.text("δ;$(nope)");
    variadic.key(Key::Enter);
    variadic.wait("third variadic prompt", "value 3");
    variadic.key(Key::Enter);
    variadic.confirm();
    let status = variadic.exit();
    assert_eq!(
        status.exit_code(),
        0,
        "variadic events={:?}",
        variadic.events()
    );
    assert_eq!(variadic.events(), ["variadic:first item:δ;$(nope)"]);

    let mut plus = RealTvScenario::launch("plus-variadic").unwrap();
    plus.select_recipe("plus-variadic");
    plus.key(Key::Enter);
    plus.wait("required plus value", "value 1");
    plus.text("required item");
    plus.key(Key::Enter);
    plus.wait("optional second plus value", "value 2");
    plus.key(Key::Enter);
    plus.confirm();
    let status = plus.exit();
    assert_eq!(status.exit_code(), 0, "plus events={:?}", plus.events());
    assert_eq!(plus.events(), ["plus:required item"]);

    let mut flags = RealTvScenario::launch("flags-and-options").unwrap();
    flags.select_recipe("flag-options");
    flags.key(Key::Enter);
    flags.wait("fixed-value flag picker", "true");
    flags.text("true");
    flags.key(Key::Enter);
    flags.wait("value-taking option prompt", "target");
    flags.text("production");
    flags.key(Key::Enter);
    flags.confirm();
    let status = flags.exit();
    assert_eq!(status.exit_code(), 0, "flag events={:?}", flags.events());
    assert_eq!(flags.events(), ["flags:enabled:production"]);
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn nested_choice_boolean_file_and_directory_pickers() {
    let mut choice = RealTvScenario::launch("choice-picker").unwrap();
    choice.select_recipe("choose");
    choice.key(Key::Enter);
    choice.wait("choice picker", "development");
    choice.text("staging");
    choice.wait("filtered choice", "staging");
    choice.key(Key::Enter);
    choice.confirm();
    assert_status(choice.exit(), 0);
    assert_eq!(choice.events(), ["choose:staging"]);

    let mut boolean = RealTvScenario::launch("boolean-picker").unwrap();
    boolean.select_recipe("boolean");
    boolean.key(Key::Enter);
    boolean.wait("boolean picker", "true");
    boolean.text("false");
    boolean.key(Key::Enter);
    boolean.confirm();
    assert_status(boolean.exit(), 0);
    assert_eq!(boolean.events(), ["boolean:false"]);

    let mut file = RealTvScenario::launch("file-picker").unwrap();
    file.select_recipe("pick-file");
    file.key(Key::Enter);
    file.wait("file picker", "sample.txt");
    file.text("sample.txt");
    file.key(Key::Enter);
    file.confirm();
    assert_status(file.exit(), 0);
    let file_event = file.events().pop().unwrap();
    assert!(file_event.ends_with("/sample.txt"), "event={file_event:?}");

    let mut directory = RealTvScenario::launch("directory-picker").unwrap();
    directory.select_recipe("pick-directory");
    directory.key(Key::Enter);
    directory.wait("directory picker", "sample-directory");
    directory.text("sample-directory");
    directory.key(Key::Enter);
    directory.confirm();
    assert_status(directory.exit(), 0);
    let directory_event = directory.events().pop().unwrap();
    assert!(
        directory_event.ends_with("/sample-directory"),
        "event={directory_event:?}"
    );

    let mut newline = RealTvScenario::launch("newline-choice").unwrap();
    newline.select_recipe("newline-choice");
    newline.key(Key::Enter);
    newline.wait("newline choice picker", "line1 line2");
    newline.key(Key::Enter);
    newline.confirm();
    assert_status(newline.exit(), 0);
    assert_eq!(newline.events(), ["newline:6c696e65310a6c696e6532"]);
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn workspace_sources_cycle_and_subfolder_recipe_runs_in_its_directory() {
    let mut scenario = RealTvScenario::launch_workspace("workspace-source-cycle").unwrap();
    let root = scenario.wait("initial root source recipe", "simple");
    assert!(root.contains("Root"));
    assert!(!root.contains("supabase/"));
    let launch_invocations = scenario.just_invocations();
    assert_eq!(
        launch_invocations
            .iter()
            .filter(|argv| argv.contains("--dump"))
            .count(),
        4,
        "expected one compatibility dump, one root dump, and one dump per standalone child: {launch_invocations:?}"
    );
    assert_eq!(
        launch_invocations
            .iter()
            .filter(|argv| argv.contains("supabase/justfile"))
            .count(),
        1,
        "subfolder Justfile must be loaded once: {launch_invocations:?}"
    );
    assert_eq!(
        launch_invocations
            .iter()
            .filter(|argv| argv.contains("tools.just"))
            .count(),
        1,
        "named *.just target must be loaded once: {launch_invocations:?}"
    );
    assert!(
        launch_invocations
            .iter()
            .all(|argv| !argv.contains("--justfile") || !argv.contains("ops.just")),
        "root module source must not be loaded again as a standalone target: {launch_invocations:?}"
    );

    let subfolders = scenario.cycle_source("Subfolders");
    assert!(subfolders.contains("📁 supabase/"), "{subfolders:?}");
    assert!(subfolders.contains("📁 tools.just"), "{subfolders:?}");

    let modules = scenario.cycle_source("Modules");
    assert!(modules.contains("ops::module-run"), "{modules:?}");

    let all = scenario.cycle_source("All");
    assert!(all.contains("simple"));
    assert!(all.contains("📁 supabase/"));
    assert!(all.contains("ops::module-run"));
    assert_eq!(
        scenario.just_invocations(),
        launch_invocations,
        "source cycling must only filter the cached workspace catalog"
    );

    scenario.cycle_source("Root");
    scenario.cycle_source("Subfolders");
    scenario.select_recipe("workspace-cwd");
    scenario.key(Key::Enter);
    scenario.confirm();
    assert_status(scenario.exit(), 0);
    let event = scenario.events().pop().expect("workspace cwd event");
    assert!(event.starts_with("workspace-cwd:"), "event={event}");
    assert!(event.ends_with("/supabase"), "event={event}");
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn child_execution_returns_to_the_interactive_shell_startup_directory() {
    use pty::{PtyCommand, PtySession};

    scenarios::require_real_tools().unwrap();
    let mut setup = RealTvScenario::launch_workspace("workspace-parent-cwd-setup").unwrap();
    setup.session.terminate().unwrap();
    let sandbox = &setup.sandbox;
    let project = sandbox.project().display().to_string();
    let command_line = format!("'{}'", env!("CARGO_BIN_EXE_jtv"));
    let mut command = PtyCommand::new("/bin/bash", sandbox.project())
        .arg("--noprofile")
        .arg("--norc")
        .arg("-i")
        .env("PS1", "JTV-SHELL> ")
        .viewport(160, 40);
    for (key, value) in sandbox.environment() {
        command = command.env(key, value);
    }
    let host_path = std::env::var_os("PATH").unwrap();
    let jtv_dir = std::path::Path::new(env!("CARGO_BIN_EXE_jtv"))
        .parent()
        .unwrap();
    let callback_path = std::env::join_paths(
        std::iter::once(jtv_dir.to_path_buf()).chain(std::env::split_paths(&host_path)),
    )
    .unwrap();
    command = command
        .env("PATH", callback_path)
        .env("JTV_JUST", scenarios::real_tool_path("just").unwrap())
        .env("JTV_TV", scenarios::real_tool_path("tv").unwrap())
        .env("JTV_E2E_LOG", sandbox.root().join("executions.log"));
    let mut shell = PtySession::spawn(command).unwrap();
    shell
        .wait_for_screen("shell prompt", Duration::from_secs(10), |f| {
            f.contains("JTV-SHELL>")
        })
        .unwrap();
    shell.send_text(&command_line).unwrap();
    shell.send_key(Key::Enter).unwrap();
    shell
        .wait_for_screen("root source", Duration::from_secs(15), |f| {
            f.contains("jtv-recipes") && f.contains("Root")
        })
        .unwrap();
    shell.send_key(Key::Ctrl('s')).unwrap();
    shell
        .wait_for_screen("subfolders source", Duration::from_secs(15), |f| {
            f.contains("Subfolders") && f.contains("supabase/")
        })
        .unwrap();
    shell.send_text("workspace-cwd").unwrap();
    shell
        .wait_for_screen("child recipe", Duration::from_secs(15), |f| {
            f.contains("workspace-cwd")
        })
        .unwrap();
    shell.send_key(Key::Enter).unwrap();
    shell
        .wait_for_screen("confirmation", Duration::from_secs(15), |f| {
            f.contains("Run selected recipe(s)?")
        })
        .unwrap();
    shell.send_key(Key::Enter).unwrap();
    shell
        .wait_for_screen("restored shell", Duration::from_secs(15), |f| {
            f.contains("JTV-SHELL>")
        })
        .unwrap();
    shell
        .send_text("printf 'shell-cwd:%s\\n' \"$PWD\"")
        .unwrap();
    shell.send_key(Key::Enter).unwrap();
    shell
        .wait_for_screen("unchanged shell cwd", Duration::from_secs(10), |f| {
            f.contains(&format!("shell-cwd:{project}"))
        })
        .unwrap();
    shell.send_text("exit").unwrap();
    shell.send_key(Key::Enter).unwrap();
    assert_status(shell.wait_for_exit(Duration::from_secs(10)).unwrap(), 0);
    let events = fs::read_to_string(sandbox.root().join("executions.log")).unwrap();
    assert!(
        events.lines().any(|line| line.ends_with("/supabase")),
        "child recipe did not observe its Justfile directory: {events:?}"
    );
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn cross_source_marks_keep_duplicate_recipe_identity_and_run_in_catalog_order() {
    let mut scenario = RealTvScenario::launch_workspace("workspace-cross-source-marks").unwrap();
    scenario.select_recipe("simple");
    scenario.key(Key::Tab);
    scenario.cycle_source("Subfolders");
    scenario.wait("same-named child recipe", "supabase/");
    scenario.key(Key::Tab);
    scenario.key(Key::Enter);
    scenario.confirm();
    assert_status(scenario.exit(), 0);
    let events = scenario.events();
    assert_eq!(events.len(), 2, "events={events:?}");
    assert_eq!(events[0], "simple");
    assert!(
        events[1].starts_with("subfolder-simple:") && events[1].ends_with("/supabase"),
        "events={events:?}"
    );
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn tab_completes_an_ordinary_string_with_a_recursive_relative_path() {
    let mut scenario = RealTvScenario::launch("tab-path-completion").unwrap();
    scenario
        .sandbox
        .write_project_file("docs/guide.md", b"guide\n")
        .unwrap();
    scenario.select_recipe("capture");
    scenario.key(Key::Enter);
    scenario.wait("ordinary string prompt with completion hint", "TAB files");
    scenario.text("docs/guide");
    scenario.key(Key::Tab);
    scenario.wait("nested recursive path picker", "docs/guide.md");
    scenario.key(Key::Enter);
    scenario
        .session
        .wait_for_screen(
            "selected relative path restored to prompt",
            Duration::from_secs(10),
            |frame| {
                !frame.alternate_screen
                    && frame.contains("TAB files")
                    && frame.contains("docs/guide.md")
            },
        )
        .unwrap();
    scenario.key(Key::Enter);
    scenario.confirm();
    assert_status(scenario.exit(), 0);
    assert_eq!(scenario.events(), ["capture:docs/guide.md"]);
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn escape_from_tab_picker_preserves_partial_input_and_restores_the_prompt() {
    let mut scenario = RealTvScenario::launch("tab-path-cancel").unwrap();
    scenario
        .sandbox
        .write_project_file("docs/guide.md", b"guide\n")
        .unwrap();
    scenario.select_recipe("capture");
    scenario.key(Key::Enter);
    scenario.wait("ordinary string prompt with completion hint", "TAB files");
    scenario.text("docs");
    scenario.key(Key::Tab);
    scenario.wait("nested recursive path picker", "docs/guide.md");
    scenario.key(Key::Escape);
    scenario
        .session
        .wait_for_screen(
            "same prompt and buffer after nested cancellation",
            Duration::from_secs(10),
            |frame| {
                !frame.alternate_screen && frame.contains("TAB files") && frame.contains("docs")
            },
        )
        .unwrap();
    scenario.text("-manual");
    scenario.key(Key::Enter);
    scenario.confirm();
    assert_status(scenario.exit(), 0);
    assert_eq!(scenario.events(), ["capture:docs-manual"]);
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn secret_is_hidden_and_confirmation_is_redacted() {
    let mut scenario = RealTvScenario::launch("secret-redaction").unwrap();
    let secret = "jtv-SECRET-sentinel-4381";
    scenario.add_secret(secret);
    scenario.select_recipe("secret");
    scenario.key(Key::Enter);
    scenario.wait("secret prompt", "token");
    scenario
        .session
        .wait_for_quiet(Duration::from_millis(50), Duration::from_secs(2))
        .unwrap();
    scenario.secret(secret);
    scenario.key(Key::Enter);
    let confirmation = scenario.wait("redacted confirmation", "[REDACTED]");
    assert!(!confirmation.contains(secret));
    assert!(!String::from_utf8_lossy(&scenario.session.transcript()).contains(secret));
    scenario.key(Key::Enter);
    assert_status(scenario.exit(), 0);
    let events = scenario.events();
    assert!(
        events.len() == 1
            && events[0]
                .strip_prefix("secret:")
                .is_some_and(|value| value == secret),
        "secret recipe event did not preserve the supplied value"
    );
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn escape_cancels_root_nested_and_confirmation() {
    let mut root = RealTvScenario::launch("escape-root").unwrap();
    root.wait("root browser", "jtv-recipes");
    root.key(Key::Escape);
    assert_status(root.exit(), 0);
    assert!(root.events().is_empty());

    let mut nested = RealTvScenario::launch("escape-nested").unwrap();
    nested.select_recipe("choose");
    nested.key(Key::Enter);
    nested.wait("nested picker", "development");
    nested.key(Key::Escape);
    let status = nested.exit();
    assert_ne!(
        status.exit_code(),
        0,
        "nested cancellation must not look like execution"
    );
    assert!(nested.events().is_empty());
    nested.assert_clean();

    let mut decline = RealTvScenario::launch("decline-confirmation").unwrap();
    decline.select_recipe("simple");
    decline.key(Key::Enter);
    decline.wait("confirmation prompt", "Run selected recipe(s)?");
    decline.text("n");
    decline.key(Key::Enter);
    assert_status(decline.exit(), 0);
    assert!(decline.events().is_empty());
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn ctrl_c_root_and_nested_leave_no_state() {
    let mut root = RealTvScenario::launch("ctrl-c-root").unwrap();
    root.wait("root browser", "jtv-recipes");
    root.session.interrupt().unwrap();
    // Television handles Ctrl-C as an orderly cancellation and returns zero.
    assert_status(root.exit(), 0);
    assert!(root.events().is_empty());
    root.assert_clean();

    let mut nested = RealTvScenario::launch("ctrl-c-nested").unwrap();
    nested.select_recipe("choose");
    nested.key(Key::Enter);
    nested.wait("nested picker", "development");
    nested.session.interrupt().unwrap();
    assert_status(nested.exit(), 130);
    assert!(nested.events().is_empty());
    nested.assert_clean();

    let mut prompt = RealTvScenario::launch("ctrl-c-prompt").unwrap();
    prompt.select_recipe("capture");
    prompt.key(Key::Enter);
    prompt.wait("scalar prompt", "value");
    prompt.session.interrupt().unwrap();
    assert_status(prompt.exit(), 130);
    assert!(prompt.events().is_empty());
    prompt.assert_clean();

    let mut secret = RealTvScenario::launch("ctrl-c-secret-prompt").unwrap();
    secret.select_recipe("secret");
    secret.key(Key::Enter);
    secret.wait("secret prompt", "token");
    secret.session.interrupt().unwrap();
    assert_status(secret.exit(), 130);
    assert!(secret.events().is_empty());
    secret.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn sigterm_root_and_nested_reap_the_process_group() {
    for (name, nested) in [("sigterm-root", false), ("sigterm-nested", true)] {
        let mut scenario = RealTvScenario::launch(name).unwrap();
        scenario.wait("root recipe browser", "jtv-recipes");
        if nested {
            scenario.text("boolean");
            scenario.key(Key::Enter);
            scenario.wait("nested boolean picker", "true");
        }
        let group = scenario
            .session
            .process_group_leader()
            .expect("Unix PTY process group leader");
        let status = Command::new("kill")
            .args(["-TERM", &format!("-{group}")])
            .status()
            .expect("send SIGTERM to PTY process group");
        assert!(status.success(), "kill failed for process group {group}");
        assert_status(scenario.exit(), 130);
        wait_for_process_group_exit(group);
        scenario.assert_clean();
        assert!(scenario.events().is_empty());
    }
}

fn wait_for_process_group_exit(group: i32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let alive = Command::new("kill")
            .args(["-0", &format!("-{group}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !alive {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "PTY process group {group} survived SIGTERM"
        );
        thread::yield_now();
    }
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn multi_select_is_deterministic_and_stops_on_first_failure() {
    let mut scenario = RealTvScenario::launch("multi-first-failure").unwrap();
    scenario.select_recipe("queue-");
    scenario.key(Key::Tab);
    scenario.wait("first marked recipe", "queue-");
    scenario.key(Key::Tab);
    scenario.key(Key::Tab);
    scenario.key(Key::Enter);
    scenario.confirm();
    assert_status(scenario.exit(), 7);
    assert_eq!(scenario.events(), ["queue-a", "queue-b-fail"]);
    scenario.assert_clean();
}

#[test]
#[ignore = "requires pinned real television and just; run via test-tui"]
fn terminal_is_usable_after_interrupt() {
    use pty::{PtyCommand, PtySession};

    scenarios::require_real_tools().unwrap();
    let mut scenario = RealTvScenario::launch("terminal-recovery-setup").unwrap();
    scenario.session.terminate().unwrap();
    let sandbox = &scenario.sandbox;
    let jtv = env!("CARGO_BIN_EXE_jtv");
    let command_line = format!("'{jtv}' --justfile justfile");
    let mut command = PtyCommand::new("/bin/bash", sandbox.project())
        .arg("--noprofile")
        .arg("--norc")
        .arg("-i")
        .env("PS1", "JTV-SHELL> ")
        .viewport(160, 40);
    for (key, value) in sandbox.environment() {
        command = command.env(key, value);
    }
    command = command
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("JTV_JUST", scenarios::real_tool_path("just").unwrap())
        .env("JTV_TV", scenarios::real_tool_path("tv").unwrap())
        .env("JTV_E2E_LOG", sandbox.root().join("executions.log"));
    let mut shell = PtySession::spawn(command).unwrap();
    shell
        .wait_for_screen("shell prompt", Duration::from_secs(10), |f| {
            f.contains("JTV-SHELL>")
        })
        .unwrap();
    shell.send_text(&command_line).unwrap();
    shell.send_key(Key::Enter).unwrap();
    shell
        .wait_for_screen("root TV", Duration::from_secs(15), |f| {
            f.contains("jtv-recipes")
        })
        .unwrap();
    shell.interrupt().unwrap();
    shell
        .wait_for_screen("restored shell", Duration::from_secs(15), |f| {
            f.contains("JTV-SHELL>")
        })
        .unwrap();
    shell.send_text("printf 'terminal-ready\\n'").unwrap();
    shell.send_key(Key::Enter).unwrap();
    shell
        .wait_for_screen("subsequent command", Duration::from_secs(10), |f| {
            f.contains("terminal-ready")
        })
        .unwrap();
    shell.send_text("exit").unwrap();
    shell.send_key(Key::Enter).unwrap();
    assert_status(shell.wait_for_exit(Duration::from_secs(10)).unwrap(), 0);
    assert!(
        fs::read_dir(sandbox.runtime())
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                !name.starts_with("jtv-session-") && !name.starts_with("jtv-picker-")
            })
    );
}
