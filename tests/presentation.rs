use jtv::presentation::{
    ColorMode, Icon, IconMode, ResolvedColorMode, ResolvedIconMode, StyleRole, StyledSpan,
    StyledText, sanitize_inline, sanitize_multiline, validate_sgr_only,
};

#[test]
fn archived_palette_is_exact() {
    let cases = [
        (StyleRole::Plain, ""),
        (StyleRole::Recipe, "\x1b[0;36m"),
        (StyleRole::ParameterName, "\x1b[1;33m"),
        (StyleRole::Required, "\x1b[0;91m"),
        (StyleRole::DefaultValue, "\x1b[0;32m"),
        (StyleRole::Dependency, "\x1b[0;95m"),
        (StyleRole::ModuleHeader, "\x1b[1;37m"),
        (StyleRole::ModuleDocker, "\x1b[0;94m"),
        (StyleRole::ModuleTest, "\x1b[0;92m"),
        (StyleRole::ModuleDeploy, "\x1b[0;93m"),
        (StyleRole::ModuleDefault, "\x1b[0;96m"),
        (StyleRole::PreviewTitle, "\x1b[1;32m"),
        (StyleRole::Documentation, "\x1b[0;35m"),
        (StyleRole::Attribute, "\x1b[0;94m"),
        (StyleRole::Signature, "\x1b[0;36m"),
        (StyleRole::Dim, "\x1b[2m"),
        (StyleRole::Bold, "\x1b[1m"),
        (StyleRole::Success, "\x1b[0;32m"),
        (StyleRole::Warning, "\x1b[0;93m"),
        (StyleRole::Error, "\x1b[0;31m"),
    ];
    for (role, sgr) in cases {
        assert_eq!(role.sgr(), sgr, "{role:?}");
    }
}

#[test]
fn plain_and_ansi_have_identical_visible_text_and_no_bleed() {
    let mut text = StyledText::new();
    text.inline("build", StyleRole::Recipe)
        .inline(" ", StyleRole::Plain)
        .inline("target", StyleRole::ParameterName);
    assert_eq!(text.plain(), "build target");
    assert_eq!(
        text.ansi(),
        "\x1b[0;36mbuild\x1b[0m \x1b[1;33mtarget\x1b[0m"
    );
    assert_eq!(text.render(ResolvedColorMode::Plain), text.plain());
    assert_eq!(text.render(ResolvedColorMode::Color), text.ansi());
    assert!(text.ansi().ends_with("\x1b[0m"));
    assert_eq!(StyledText::new().ansi(), "\x1b[0m");
}

#[test]
fn hostile_controls_are_neutralized_but_safe_unicode_survives() {
    let hostile = "café界e\u{301}\x1b[31mRED\x1b[0m\x1b]52;c;pw\x07!\r\u{009b}2J\u{202e}x\u{0007}";
    let clean = sanitize_inline(hostile);
    assert_eq!(clean, "café界é�RED��!���x�");
    assert!(!clean.contains('\x1b'));
    assert!(!clean.contains('\u{202e}'));
}

#[test]
fn inline_and_multiline_control_policy_is_strict() {
    assert_eq!(sanitize_inline("a\tb\nc"), "a b c");
    assert_eq!(sanitize_multiline("a\tb\nc\rd"), "a b\nc�d");
}

#[test]
fn span_constructors_sanitize_untrusted_text() {
    let span = StyledSpan::inline("ok\x1b[2Jbad", StyleRole::Recipe);
    assert_eq!(span.text(), "ok�bad");
    assert_eq!(span.role(), StyleRole::Recipe);
    assert_eq!(StyledText::from(span).plain(), "ok�bad");
}

#[test]
fn sgr_validator_accepts_only_sgr_and_safe_text() {
    let safe = "\x1b[1;38;5;214mλ\x1b[0m\n\x1b[38:2:1:2:3m界\x1b[m";
    assert_eq!(validate_sgr_only(safe), Ok(safe));
    for hostile in [
        "\x1b]8;;https://evil\x07x",
        "\x1b[2J",
        "\x1b[1A",
        "bell\x07",
        "\u{009b}31mred",
        "\u{202e}abc",
        "\x1b[1;;2m",
        "\x1b[999m",
        "\x1b[38;5;999m",
        "\x1b[38;2;1;2m",
    ] {
        assert!(validate_sgr_only(hostile).is_err(), "accepted {hostile:?}");
    }
}

fn env_of<'a>(values: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |key| {
        values
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| (*value).to_owned())
    }
}

#[test]
fn color_resolution_honors_environment_and_explicit_overrides() {
    assert_eq!(
        ColorMode::Auto.resolve_with(env_of(&[])),
        ResolvedColorMode::Color
    );
    assert_eq!(
        ColorMode::Auto.resolve_with(env_of(&[("NO_COLOR", "")])),
        ResolvedColorMode::Plain
    );
    assert_eq!(
        ColorMode::Auto.resolve_with(env_of(&[("TERM", "dumb")])),
        ResolvedColorMode::Plain
    );
    assert_eq!(
        ColorMode::Always.resolve_with(env_of(&[("NO_COLOR", "1"), ("TERM", "dumb")])),
        ResolvedColorMode::Color
    );
    assert_eq!(
        ColorMode::Never.resolve_with(env_of(&[])),
        ResolvedColorMode::Plain
    );
}

#[test]
fn icon_resolution_honors_locale_legacy_switch_and_explicit_overrides() {
    assert_eq!(
        IconMode::Auto.resolve_with(env_of(&[("LANG", "en_US.UTF-8")])),
        ResolvedIconMode::Unicode
    );
    assert_eq!(
        IconMode::Auto.resolve_with(env_of(&[("LC_ALL", "C"), ("LANG", "en_US.UTF-8")])),
        ResolvedIconMode::Ascii
    );
    assert_eq!(
        IconMode::Auto.resolve_with(env_of(&[("NO_ICONS", "1"), ("LANG", "en_US.UTF-8")])),
        ResolvedIconMode::Ascii
    );
    assert_eq!(
        IconMode::Auto.resolve_with(env_of(&[("TERM", "dumb"), ("LANG", "en_US.UTF-8")])),
        ResolvedIconMode::Ascii
    );
    assert_eq!(
        IconMode::Unicode.resolve_with(env_of(&[("NO_ICONS", "1")])),
        ResolvedIconMode::Unicode
    );
    assert_eq!(
        IconMode::None.resolve_with(env_of(&[("LANG", "en_US.UTF-8")])),
        ResolvedIconMode::None
    );
}

#[test]
fn icon_vocabulary_and_fallbacks_are_complete() {
    let cases = [
        (Icon::Standalone, "▶", "[recipe]"),
        (Icon::Core, "🔷", "[core]"),
        (Icon::Docker, "🐳", "[docker]"),
        (Icon::Test, "🧪", "[test]"),
        (Icon::Deploy, "🚀", "[deploy]"),
        (Icon::Module, "📦", "[mod]"),
    ];
    for (icon, unicode, ascii) in cases {
        assert_eq!(icon.render(ResolvedIconMode::Unicode), unicode);
        assert_eq!(icon.render(ResolvedIconMode::Ascii), ascii);
        assert_eq!(icon.render(ResolvedIconMode::None), "");
    }
    assert_eq!(Icon::for_module(None, false), Icon::Standalone);
    assert_eq!(Icon::for_module(None, true), Icon::Core);
    assert_eq!(Icon::for_module(Some("testing"), true), Icon::Test);
    assert_eq!(Icon::for_module(Some("deployment"), true), Icon::Deploy);
    assert_eq!(Icon::for_module(Some("custom"), true), Icon::Module);
}

#[test]
fn styled_truncation_preserves_roles_and_unicode_cell_width() {
    let mut text = StyledText::new();
    text.inline("界界", StyleRole::Recipe)
        .inline("abcdef", StyleRole::ParameterName);
    let truncated = text.truncate(6);
    assert_eq!(console::measure_text_width(&truncated.plain()), 6);
    assert_eq!(truncated.plain(), "界界a…");
    assert_eq!(truncated.spans()[0].role(), StyleRole::Recipe);
    assert_eq!(truncated.spans().last().unwrap().role(), StyleRole::Dim);
}
