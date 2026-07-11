#[path = "support/fake_tools.rs"]
mod fake_tools;
#[allow(dead_code)]
#[path = "support/sandbox.rs"]
mod sandbox;

use std::{fs, process::Output};

use fake_tools::FakeTools;

fn launch(tools: &FakeTools, mode: &str) -> Output {
    tools
        .jtv_command()
        .env("JTV_TEST_TV_MODE", mode)
        .output()
        .expect("launch jtv")
}

#[test]
fn probes_versions_and_records_top_level_argv_cwd_and_session_environment() {
    let tools = FakeTools::new();
    tools.init();
    let output = launch(&tools, "cancel");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let events = tools.records();
    assert!(events.contains("just\t--version\n"));
    assert!(events.contains("tv\t--version\n"));
    assert!(events.contains("tv\tjtv-recipes\t"));
    assert!(events.contains("\t--cable-dir\t"));
    assert!(events.contains("\t--no-remote\n"));
    let environment = events
        .lines()
        .find(|line| line.starts_with("tv-env\t"))
        .expect("TV environment record");
    assert!(environment.contains(&tools.sandbox.project().display().to_string()));
    assert!(environment.contains("jtv-session-"));
}

#[test]
fn doctor_accepts_supported_versions_and_rejects_each_unsupported_probe() {
    let supported = FakeTools::new();
    supported.init();
    assert!(
        supported
            .jtv_command()
            .arg("doctor")
            .status()
            .unwrap()
            .success()
    );

    let old_just = FakeTools::new();
    old_just.init();
    let output = old_just
        .jtv_command()
        .arg("doctor")
        .env("JTV_TEST_JUST_VERSION", "1.52.0")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("[FAIL] just 1.52.0"));

    let old_tv = FakeTools::new();
    old_tv.init();
    let output = old_tv
        .jtv_command()
        .arg("doctor")
        .env("JTV_TEST_TV_VERSION", "0.15.8")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("[FAIL] television 0.15.8"));
}

#[test]
fn fake_just_records_exact_os_arguments_show_output_and_status() {
    let tools = FakeTools::new();
    let output = tools
        .sandbox
        .command(tools.just())
        .args(["--show", "name with spaces;$(literal)"])
        .env("JTV_TEST_RECORD", &tools.record)
        .env("JTV_TEST_JUST_SHOW", "literal preview\n")
        .env("JTV_TEST_JUST_SHOW_STATUS", "19")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(19));
    assert_eq!(output.stdout, b"literal preview\n");
    assert!(
        tools
            .records()
            .contains("just\t--show\tname with spaces;$(literal)\n")
    );
}

#[test]
fn source_preview_and_one_action_use_only_opaque_callback_ids() {
    let tools = FakeTools::new();
    tools.init();
    let output = launch(&tools, "one");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let events = tools.records();
    let source = events
        .lines()
        .find(|line| line.starts_with("source-output\t"))
        .expect("source callback output");
    let ids = recorded_source_ids(source);
    assert_eq!(ids.len(), 2);
    assert!(source.contains("alpha") && source.contains("beta"));
    assert!(events.contains(&format!("callback\t__tv-preview\t{}", ids[0])));
    assert!(events.contains(&format!("action-callback\t__tv-run\t{}", ids[0])));
    assert!(events.contains("preview-output\t") && events.contains("alpha"));
}

#[test]
fn released_tv_falls_back_to_plain_rows_but_patched_tv_can_opt_into_ansi() {
    let released = FakeTools::new();
    released.init();
    let output = released
        .jtv_command()
        .args(["--color", "always"])
        .env("JTV_TEST_TV_MODE", "one")
        .output()
        .unwrap();
    assert!(output.status.success());
    let released_records = released.records();
    let released_source = released_records
        .lines()
        .find(|line| line.starts_with("source-output\t"))
        .unwrap();
    assert!(!released_source.contains('\x1b'));
    assert!(released_records.contains("preview-output\t\x1b["));

    let patched = FakeTools::new();
    patched.init();
    let output = patched
        .jtv_command()
        .args(["--color", "always"])
        .env("JTV_UNSAFE_TV_ANSI_DISPLAY", "1")
        .env("JTV_TEST_TV_MODE", "one")
        .output()
        .unwrap();
    assert!(output.status.success());
    let patched_records = patched.records();
    let patched_source = patched_records
        .lines()
        .find(|line| line.starts_with("source-output\t"))
        .unwrap();
    assert!(patched_source.contains("\x1b[0;36m"));
}

#[test]
fn hostile_display_metadata_is_sanitized_and_never_used_as_an_action_key() {
    let tools = FakeTools::new();
    tools.init();
    let dump = r#"{"recipes":{"deploy":{"name":"deploy","namepath":"deploy","doc":"tabs\tquotes ' \" Unicode λ newline\n$(touch nope);","body":["echo safe"]}}}"#;
    let output = tools
        .jtv_command()
        .env("JTV_TEST_JUST_DUMP", dump)
        .env("JTV_TEST_TV_MODE", "one")
        .output()
        .unwrap();
    assert!(output.status.success());
    let events = tools.records();
    let source = events
        .lines()
        .find(|line| line.starts_with("source-output\t"))
        .unwrap();
    assert!(source.contains("Unicode λ"));
    assert!(!source.contains("\\tquotes"));
    let id = &recorded_source_ids(source)[0];
    assert!(events.contains(&format!("action-callback\t__tv-run\t{id}")));
    assert!(!events.contains("action-callback\t__tv-run\tdeploy"));
}

#[test]
fn unordered_many_are_normalized_and_duplicate_ids_execute_once() {
    let tools = FakeTools::new();
    tools.init();
    assert!(launch(&tools, "many").status.success());
    let events = tools.records();
    let source = events
        .lines()
        .find(|line| line.starts_with("source-output\t"))
        .unwrap();
    let ids = recorded_source_ids(source);
    assert!(events.contains(&format!(
        "action-callback\t__tv-run\t{}\t{}",
        ids[1], ids[0]
    )));

    let duplicate = FakeTools::new();
    duplicate.init();
    assert!(launch(&duplicate, "duplicate").status.success());
    let events = duplicate.records();
    let source = events
        .lines()
        .find(|line| line.starts_with("source-output\t"))
        .unwrap();
    let id = &recorded_source_ids(source)[0];
    assert!(events.contains(&format!("action-callback\t__tv-run\t{id}\t{id}")));
}

fn recorded_source_ids(source_event: &str) -> Vec<String> {
    source_event
        .strip_prefix("source-output\t")
        .expect("source output event")
        .split("\\n")
        .filter(|row| !row.is_empty())
        .filter_map(|row| row.split("\\t").next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn cancellation_empty_unknown_and_malformed_selections_execute_nothing() {
    for mode in ["cancel", "empty-action", "unknown", "malformed"] {
        let tools = FakeTools::new();
        tools.init();
        let output = launch(&tools, mode);
        if mode == "cancel" {
            assert!(output.status.success());
        } else {
            assert!(!output.status.success(), "{mode} unexpectedly succeeded");
        }
        assert!(
            !tools
                .records()
                .lines()
                .any(|line| line == "just\talpha" || line == "just\tbeta"),
            "{mode} executed a recipe"
        );
    }
}

#[test]
fn action_and_television_statuses_propagate_exactly() {
    let tools = FakeTools::new();
    tools.init();
    let output = tools
        .jtv_command()
        .env("JTV_TEST_TV_MODE", "one")
        .env("JTV_TEST_TV_ACTION_STATUS", "23")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(23));

    let cancelled = FakeTools::new();
    cancelled.init();
    let output = cancelled
        .jtv_command()
        .env("JTV_TEST_TV_MODE", "cancel")
        .env("JTV_TEST_TV_CANCEL_STATUS", "42")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn nested_picker_supports_result_cancel_unknown_and_malformed_output() {
    for (picker_mode, expected_output) in [
        ("select", "pick-00000000"),
        ("cancel", ""),
        ("unknown", "pick-ffffffff"),
        ("malformed", "display text; $(not executable)"),
    ] {
        let tools = FakeTools::new();
        let picker_state = tools.sandbox.root().join("picker.json");
        fs::write(
            &picker_state,
            r#"{"entries":[{"id":"pick-00000000","display":"dev","value":"dev"},{"id":"pick-00000001","display":"prod","value":"prod"}]}"#,
        )
        .unwrap();
        let output = tools
            .sandbox
            .command(tools.tv())
            .args(["--source-command", "ignored"])
            .env("JTV_TEST_RECORD", &tools.record)
            .env("JTV_TEST_JTV_BIN", tools.jtv())
            .env("JTV_PICKER_STATE", picker_state)
            .env("JTV_TEST_PICKER_MODE", picker_mode)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected_output
        );
        let records = tools.records();
        assert!(records.contains("picker-source-output\tpick-00000000\\tdev"));
    }
}

#[test]
fn nested_picker_nonzero_status_is_exact() {
    let tools = FakeTools::new();
    let picker_state = tools.sandbox.root().join("picker.json");
    fs::write(&picker_state, r#"{"entries":[]}"#).unwrap();
    let output = tools
        .sandbox
        .command(tools.tv())
        .args(["--source-command", "ignored"])
        .env("JTV_TEST_RECORD", &tools.record)
        .env("JTV_TEST_JTV_BIN", tools.jtv())
        .env("JTV_PICKER_STATE", picker_state)
        .env("JTV_TEST_PICKER_STATUS", "17")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17));
}

#[test]
fn nested_picker_selects_a_requested_display_but_returns_only_its_opaque_id() {
    let tools = FakeTools::new();
    let picker_state = tools.sandbox.root().join("picker.json");
    fs::write(
        &picker_state,
        r#"{"entries":[{"id":"pick-00000000","display":"docs","value":"docs"},{"id":"pick-00000001","display":"docs/guide.md","value":"docs/guide.md"}]}"#,
    )
    .unwrap();
    let output = tools
        .sandbox
        .command(tools.tv())
        .args(["--source-command", "ignored", "--input", "docs"])
        .env("JTV_TEST_RECORD", &tools.record)
        .env("JTV_TEST_JTV_BIN", tools.jtv())
        .env("JTV_PICKER_STATE", picker_state)
        .env("JTV_TEST_PICKER_SELECT_DISPLAY", "docs/guide.md")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "pick-00000001"
    );
    let records = tools.records();
    assert!(records.contains("picker-argv\t--source-command\tignored\t--input\tdocs"));
    assert!(
        records
            .contains("picker-source-output\tpick-00000000\\tdocs\\npick-00000001\\tdocs/guide.md")
    );
}

#[test]
fn malformed_and_additive_just_json_are_distinguished_at_the_os_boundary() {
    let additive = FakeTools::new();
    additive.init();
    assert!(launch(&additive, "cancel").status.success());

    let malformed = FakeTools::new();
    malformed.init();
    let output = malformed
        .jtv_command()
        .env("JTV_TEST_JUST_DUMP_MODE", "malformed")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unable to parse Justfile JSON"));
    assert!(
        !malformed
            .records()
            .lines()
            .any(|line| line.starts_with("tv\tjtv-recipes"))
    );
}

#[test]
fn helper_is_feature_gated_and_not_a_default_release_binary() {
    let manifest = fs::read_to_string("Cargo.toml").unwrap();
    assert!(manifest.contains("required-features = [\"test-support\"]"));
    assert!(manifest.contains("test = false"));
    assert!(manifest.contains("bench = false"));
}
