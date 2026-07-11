//! Television process integration and row protocol.

use std::{
    fmt::Write as _,
    path::Path,
    process::{Command, ExitStatus, Stdio},
};

use clap::ValueEnum;
use semver::Version;

#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
#[cfg(unix)]
use std::{
    env,
    io::{Read as _, Write as _},
    thread,
    time::{Duration, Instant},
};

use crate::{
    Error, Result,
    channel::CHANNEL_NAME,
    config::{Config, ParameterConfig},
    just,
    model::{Parameter, ParameterKind, Recipe},
    presentation::{
        Icon, PresentationOptions, ResolvedColorMode, StyleRole, StyledText, sanitize_inline,
        sanitize_multiline, validate_sgr_only,
    },
    session::{SESSION_ENV, SessionFile, SessionState, validate_id},
    workspace::WorkspaceOrigin,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum SourceView {
    #[default]
    Root,
    Subfolders,
    Modules,
    All,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRow {
    pub id: String,
    pub display: StyledText,
    pub search: String,
}

impl SourceRow {
    pub fn encode(&self, color: ResolvedColorMode) -> Result<String> {
        validate_id(&self.id)?;
        let display = self.display.render(color);
        validate_sgr_only(&display)
            .map_err(|error| Error::Message(format!("unsafe styled source row: {error}")))?;
        Ok(format!(
            "{}\t{}\t{}",
            self.id,
            display,
            sanitize_field(&self.search)
        ))
    }
}

pub fn rows(state: &SessionState, view: SourceView) -> Vec<SourceRow> {
    let modular = state
        .primary_target()
        .map(|target| {
            target
                .project
                .recipes
                .iter()
                .any(|recipe| recipe.module.is_some())
        })
        .unwrap_or(false);
    let mut recipes = state
        .catalog
        .selections
        .iter()
        .filter_map(|(id, _)| state.resolve(id).ok().map(|resolved| (id, resolved)))
        .filter(|(_, resolved)| matches_view(&resolved.target.origin, resolved.recipe, view))
        .collect::<Vec<_>>();
    recipes.sort_by(|(_, left), (_, right)| {
        (
            origin_order(&left.target.origin),
            origin_path(&left.target.origin),
            left.recipe.module.is_some(),
            left.recipe.module.as_deref(),
            left.recipe.name.as_str(),
        )
            .cmp(&(
                origin_order(&right.target.origin),
                origin_path(&right.target.origin),
                right.recipe.module.is_some(),
                right.recipe.module.as_deref(),
                right.recipe.name.as_str(),
            ))
    });
    recipes
        .into_iter()
        .map(|(id, resolved)| SourceRow {
            id: id.clone(),
            display: recipe_row(state, resolved.target, resolved.recipe, modular),
            search: searchable(&resolved.target.origin, resolved.recipe),
        })
        .collect()
}

pub fn source_output(state: &SessionState, view: SourceView) -> Result<String> {
    let mut output = String::new();
    for row in rows(state, view) {
        writeln!(output, "{}", row.encode(state.presentation.source_color)?)
            .expect("String writes cannot fail");
    }
    Ok(output)
}

pub fn preview(state: &SessionState, id: &str) -> Result<String> {
    let resolved = state.resolve(id)?;
    Ok(details_preview(resolved.target, resolved.recipe).render(state.presentation.color))
}

pub fn definition_preview(state: &SessionState, id: &str) -> Result<String> {
    let resolved = state.resolve(id)?;
    let recipe = resolved.recipe;
    let definition =
        sanitize_multiline(&just::render_preview(&resolved.target.invocation, recipe)?);
    let mut origin = origin_preview(resolved.target);
    if state.presentation.color == ResolvedColorMode::Color && recipe.shebang {
        if let Some(highlighted) = highlight_with_bat(recipe, &definition) {
            return Ok(format!(
                "{}{highlighted}",
                origin.render(state.presentation.color)
            ));
        }
    }
    let styled = style_definition(recipe, &definition);
    for span in styled.spans() {
        origin.push(span.clone());
    }
    Ok(origin.render(state.presentation.color))
}

#[cfg(not(unix))]
fn highlight_with_bat(_recipe: &Recipe, _definition: &str) -> Option<String> {
    // A bounded process-tree kill requires platform job control. Until that is
    // implemented, deterministic internal highlighting is safer on Windows.
    None
}

#[cfg(unix)]
fn highlight_with_bat(recipe: &Recipe, definition: &str) -> Option<String> {
    const MAX_HIGHLIGHT_BYTES: u64 = 512 * 1024;
    let binary = match env::var_os("JTV_BAT") {
        Some(value) if value == "disabled" => return None,
        Some(value) => value,
        None => "bat".into(),
    };
    let mut command = Command::new(binary);
    command
        .args([
            "--paging=never",
            "--style=plain",
            "--color=always",
            "--theme=ansi",
            "--language",
            shebang_language(recipe, definition),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .by_ref()
            .take(MAX_HIGHLIGHT_BYTES + 1)
            .read_to_end(&mut bytes)
            .ok()
            .map(|_| bytes)
    });
    child
        .stdin
        .as_mut()?
        .write_all(definition.as_bytes())
        .ok()?;
    drop(child.stdin.take());
    let deadline = Instant::now() + Duration::from_millis(750);
    let status = loop {
        if let Some(status) = child.try_wait().ok()? {
            break status;
        }
        if Instant::now() >= deadline {
            // SAFETY: the child was placed in a new process group whose ID is
            // its positive PID; negating it targets only that helper group.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
            let _ = reader.join();
            return None;
        }
        thread::sleep(Duration::from_millis(10));
    };
    let bytes = reader.join().ok()??;
    if !status.success() || bytes.len() as u64 > MAX_HIGHLIGHT_BYTES {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    validate_sgr_only(&text).ok()?;
    Some(text)
}

#[cfg(unix)]
fn shebang_language(_recipe: &Recipe, definition: &str) -> &'static str {
    let shebang = definition
        .lines()
        .find(|line| line.trim_start().starts_with("#!"));
    match shebang {
        Some(line) if line.contains("python") => "python",
        Some(line) if line.contains("node") || line.contains("javascript") => "javascript",
        Some(line) if line.contains("ruby") => "ruby",
        _ => "bash",
    }
}

fn style_definition(recipe: &Recipe, definition: &str) -> StyledText {
    let mut output = StyledText::new();
    for line in definition.lines() {
        let trimmed = line.trim_start();
        let role = if trimmed.starts_with('#') && !trimmed.starts_with("#!") {
            StyleRole::Documentation
        } else if trimmed.starts_with('[') {
            StyleRole::Attribute
        } else if trimmed.starts_with(&recipe.name) || trimmed.starts_with(&recipe.namepath) {
            StyleRole::Signature
        } else {
            StyleRole::Plain
        };
        output.multiline(line, role).newline();
    }
    output
}

pub fn launch(
    tv_binary: &Path,
    cable_dir: &Path,
    cwd: &Path,
    session: &SessionFile,
    presentation: &PresentationOptions,
) -> Result<ExitStatus> {
    command(tv_binary, cable_dir, cwd, session, presentation)
        .status()
        .map_err(|source| Error::Spawn {
            program: tv_binary.display().to_string(),
            source,
        })
}

/// Construct the exact Television child command, exposed for orchestration and contract tests.
pub fn command(
    tv_binary: &Path,
    cable_dir: &Path,
    cwd: &Path,
    session: &SessionFile,
    presentation: &PresentationOptions,
) -> Command {
    let mut command = Command::new(tv_binary);
    command
        .arg(CHANNEL_NAME)
        .arg(cwd)
        .arg("--cable-dir")
        .arg(cable_dir)
        .arg("--no-remote")
        .env(SESSION_ENV, session.path())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if presentation.compact {
        command
            .arg("--layout")
            .arg("portrait")
            .arg("--preview-size")
            .arg("50");
    }
    command
}

pub fn ensure_success(status: ExitStatus) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        Err(Error::ProgramFailed {
            program: "tv".into(),
            status: status.code().unwrap_or(130),
            stderr: String::new(),
        })
    }
}

/// Probe a Television executable and return its semantic version.
pub fn version(tv_binary: &Path) -> Result<Version> {
    let output = Command::new(tv_binary)
        .arg("--version")
        .output()
        .map_err(|source| Error::Spawn {
            program: tv_binary.display().to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(Error::ProgramFailed {
            program: tv_binary.display().to_string(),
            status: output.status.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let value = text
        .split_whitespace()
        .find(|word| word.as_bytes().first().is_some_and(u8::is_ascii_digit))
        .ok_or_else(|| Error::Message(format!("unrecognized Television version output: {text}")))?;
    Version::parse(value).map_err(|error| {
        Error::Message(format!(
            "unrecognized Television version `{value}`: {error}"
        ))
    })
}

/// Compatibility floor validated by this implementation.
pub fn version_is_supported(version: &Version) -> bool {
    version >= &Version::new(0, 15, 9)
}

fn recipe_row(
    state: &SessionState,
    target: &crate::session::CatalogTarget,
    recipe: &Recipe,
    modular: bool,
) -> StyledText {
    let mut output = StyledText::new();
    let icon = if matches!(target.origin, WorkspaceOrigin::Subfolder { .. }) {
        Icon::Subfolder
    } else {
        Icon::for_module(module_root(recipe.module.as_deref()), modular)
    }
    .render(state.presentation.icons);
    if !icon.is_empty() {
        output
            .inline(icon, StyleRole::Plain)
            .inline(" ", StyleRole::Plain);
    }
    if let WorkspaceOrigin::Subfolder { label, .. } = &target.origin {
        output
            .inline(label, StyleRole::Dim)
            .inline("  ", StyleRole::Plain);
    }
    push_recipe_name(&mut output, recipe);
    let mut has_metadata = false;
    for parameter in &recipe.parameters {
        output.inline(if has_metadata { " " } else { "  " }, StyleRole::Plain);
        push_parameter_compact(
            &mut output,
            &target.config,
            recipe,
            parameter,
            state.presentation.source_color == ResolvedColorMode::Color,
        );
        has_metadata = true;
    }
    if !state.presentation.compact {
        for dependency in &recipe.dependencies {
            output
                .inline(if has_metadata { " " } else { "  " }, StyleRole::Plain)
                .inline(dependency, StyleRole::Dependency);
            has_metadata = true;
        }
    }
    output.truncate(if state.presentation.compact { 72 } else { 120 })
}

fn matches_view(origin: &WorkspaceOrigin, recipe: &Recipe, view: SourceView) -> bool {
    match view {
        SourceView::Root => matches!(origin, WorkspaceOrigin::Root) && recipe.module.is_none(),
        SourceView::Subfolders => matches!(origin, WorkspaceOrigin::Subfolder { .. }),
        SourceView::Modules => matches!(origin, WorkspaceOrigin::Root) && recipe.module.is_some(),
        SourceView::All => true,
    }
}

fn origin_order(origin: &WorkspaceOrigin) -> u8 {
    match origin {
        WorkspaceOrigin::Root => 0,
        WorkspaceOrigin::Subfolder { .. } => 1,
    }
}

fn origin_path(origin: &WorkspaceOrigin) -> &str {
    match origin {
        WorkspaceOrigin::Root => "",
        WorkspaceOrigin::Subfolder { label, .. } => label,
    }
}

fn origin_preview(target: &crate::session::CatalogTarget) -> StyledText {
    let mut output = StyledText::new();
    let (source, justfile) = match &target.origin {
        WorkspaceOrigin::Root => (
            "Root",
            target
                .invocation
                .justfile
                .as_deref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "auto-discovered".into()),
        ),
        WorkspaceOrigin::Subfolder {
            relative_justfile,
            label,
        } => (label.as_str(), relative_justfile.display().to_string()),
    };
    output
        .inline("Source: ", StyleRole::ModuleHeader)
        .inline(source, StyleRole::Attribute)
        .newline()
        .inline("Justfile: ", StyleRole::ModuleHeader)
        .inline(justfile, StyleRole::Dim)
        .newline()
        .newline();
    output
}

fn push_recipe_name(output: &mut StyledText, recipe: &Recipe) {
    if let Some(module) = &recipe.module {
        output
            .inline(module, module_style(module_root(Some(module))))
            .inline("::", StyleRole::Plain)
            .inline(&recipe.name, StyleRole::Recipe);
    } else {
        output.inline(&recipe.namepath, StyleRole::Recipe);
    }
}

fn push_parameter_compact(
    output: &mut StyledText,
    config: &Config,
    recipe: &Recipe,
    parameter: &Parameter,
    concise_colored: bool,
) {
    let spelling = parameter_spelling(parameter);
    let secret = matches!(
        config.parameter(&recipe.namepath, &parameter.name),
        Some(ParameterConfig::Secret)
    );
    if let Some(default) = &parameter.default {
        output
            .inline(&spelling, StyleRole::ParameterName)
            .inline(":", StyleRole::Plain)
            .inline(
                if secret { "[REDACTED]" } else { default },
                StyleRole::DefaultValue,
            );
    } else if parameter.default_expression.is_some() {
        output
            .inline(&spelling, StyleRole::ParameterName)
            .inline(":<just default>", StyleRole::DefaultValue);
    } else if parameter.flag && parameter.value.is_some() {
        output.inline(&spelling, StyleRole::ParameterName);
    } else if concise_colored {
        // In the colored Results language, red is the required marker. Keep
        // the token as terse as the archived interface.
        output.inline(&spelling, StyleRole::Required);
    } else {
        // Plain rows cannot rely on color, so retain an explicit text cue.
        output
            .inline(&spelling, StyleRole::Required)
            .inline(":<required>", StyleRole::Required);
    }
    match parameter.kind {
        ParameterKind::Plus => {
            output.inline("+", StyleRole::Required);
        }
        ParameterKind::Star => {
            output.inline("*", StyleRole::ParameterName);
        }
        _ => {}
    }
}

fn details_preview(target: &crate::session::CatalogTarget, recipe: &Recipe) -> StyledText {
    let mut output = origin_preview(target);
    if let Some(module) = &recipe.module {
        output
            .inline("Module: ", StyleRole::ModuleHeader)
            .inline(module, StyleRole::ModuleHeader)
            .multiline("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n", StyleRole::Dim);
    }
    output
        .inline(&recipe.namepath, StyleRole::PreviewTitle)
        .newline();
    let mut badges = Vec::new();
    if let Some(target) = &recipe.alias_target {
        badges.push(format!("alias → {target}"));
    }
    if let Some(group) = &recipe.group {
        badges.push(format!("group: {group}"));
    }
    if recipe.quiet {
        badges.push("quiet".into());
    }
    if recipe.shebang {
        badges.push("shebang".into());
    }
    if !badges.is_empty() {
        output
            .inline(badges.join("  •  "), StyleRole::Attribute)
            .newline();
    }
    if let Some(doc) = &recipe.doc {
        section_heading(&mut output, "Summary");
        output.multiline(doc, StyleRole::Documentation).newline();
    }
    if !recipe.parameters.is_empty() {
        section_heading(&mut output, "Parameters");
        for parameter in &recipe.parameters {
            push_parameter_detail(&mut output, &target.config, recipe, parameter);
        }
    }
    if !recipe.dependencies.is_empty() {
        section_heading(&mut output, "Dependencies");
        for dependency in &recipe.dependencies {
            output
                .inline("→ ", StyleRole::Dependency)
                .inline(dependency, StyleRole::Dependency)
                .newline();
        }
    }
    if !recipe.attributes.is_empty() {
        section_heading(&mut output, "Attributes");
        for attribute in &recipe.attributes {
            output
                .inline("• ", StyleRole::Attribute)
                .inline(attribute, StyleRole::Attribute)
                .newline();
        }
    }
    section_heading(&mut output, "Recipe");
    let mut signature = StyledText::new();
    push_recipe_name(&mut signature, recipe);
    for parameter in &recipe.parameters {
        signature.inline(" ", StyleRole::Plain);
        push_parameter_compact(&mut signature, &target.config, recipe, parameter, false);
    }
    output
        .inline(signature.plain(), StyleRole::Signature)
        .newline();
    for line in &recipe.body {
        output.multiline(line, StyleRole::Plain).newline();
    }
    output
}

fn section_heading(output: &mut StyledText, name: &str) {
    output.newline().inline(name, StyleRole::Bold).newline();
}

fn push_parameter_detail(
    output: &mut StyledText,
    config: &Config,
    recipe: &Recipe,
    parameter: &Parameter,
) {
    let parameter_config = config.parameter(&recipe.namepath, &parameter.name);
    let secret = matches!(parameter_config, Some(ParameterConfig::Secret));
    output
        .inline("• ", StyleRole::Plain)
        .inline(parameter_spelling(parameter), StyleRole::ParameterName);
    if let Some(default) = &parameter.default {
        output.inline(" = ", StyleRole::Plain).inline(
            if secret { "[REDACTED]" } else { default },
            StyleRole::DefaultValue,
        );
    } else if parameter.default_expression.is_some() {
        output
            .inline(" = ", StyleRole::Plain)
            .inline("<just default>", StyleRole::DefaultValue);
    } else if !(parameter.flag && parameter.value.is_some()) {
        output
            .inline(" = ", StyleRole::Plain)
            .inline("<required>", StyleRole::Required);
    }
    let mut labels = Vec::new();
    if let Some(kind) = config_kind(parameter_config) {
        labels.push(kind);
    }
    match parameter.kind {
        ParameterKind::Plus => labels.push("one or more"),
        ParameterKind::Star => labels.push("zero or more"),
        _ => {}
    }
    if !labels.is_empty() {
        output
            .inline("  [", StyleRole::Dim)
            .inline(labels.join(", "), StyleRole::Dim)
            .inline("]", StyleRole::Dim);
    }
    if let Some(help) = &parameter.help {
        output
            .inline(" — ", StyleRole::Dim)
            .inline(help, StyleRole::Dim);
    }
    output.newline();
}

fn config_kind(config: Option<&ParameterConfig>) -> Option<&'static str> {
    match config {
        Some(ParameterConfig::String) => Some("text"),
        Some(ParameterConfig::Secret) => Some("secret"),
        Some(ParameterConfig::Choice { .. }) => Some("choice"),
        Some(ParameterConfig::Boolean) => Some("boolean"),
        Some(ParameterConfig::File) => Some("file"),
        Some(ParameterConfig::Directory) => Some("directory"),
        None => None,
    }
}

fn parameter_spelling(parameter: &Parameter) -> String {
    if let Some(long) = &parameter.long {
        format!("--{long}")
    } else if let Some(short) = &parameter.short {
        format!("-{short}")
    } else {
        parameter.name.clone()
    }
}

fn module_root(module: Option<&str>) -> Option<&str> {
    module.and_then(|module| module.split("::").next())
}

fn module_style(module: Option<&str>) -> StyleRole {
    match module {
        Some("docker") => StyleRole::ModuleDocker,
        Some("test" | "testing") => StyleRole::ModuleTest,
        Some("deploy" | "deployment") => StyleRole::ModuleDeploy,
        Some(_) | None => StyleRole::ModuleDefault,
    }
}

fn searchable<'a>(origin: &'a WorkspaceOrigin, recipe: &'a Recipe) -> String {
    let mut values = vec![recipe.namepath.as_str()];
    if let WorkspaceOrigin::Subfolder {
        relative_justfile,
        label,
    } = origin
    {
        values.push(label);
        if let Some(path) = relative_justfile.to_str() {
            values.push(path);
        }
    }
    values.extend(recipe.doc.as_deref());
    values.extend(recipe.group.as_deref());
    values.extend(recipe.module.as_deref());
    values.extend(recipe.alias_target.as_deref());
    values.extend(recipe.dependencies.iter().map(String::as_str));
    values.extend(recipe.attributes.iter().map(String::as_str));
    for parameter in &recipe.parameters {
        values.push(&parameter.name);
        values.extend(parameter.long.as_deref());
        values.extend(parameter.short.as_deref());
        values.extend(parameter.help.as_deref());
    }
    sanitize_field(&values.join(" "))
}

pub fn sanitize_field(value: &str) -> String {
    sanitize_inline(value)
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        sync::Mutex,
        time::{Duration, Instant},
    };

    use super::*;

    static ENVIRONMENT: Mutex<()> = Mutex::new(());

    fn recipe() -> Recipe {
        Recipe {
            name: "script".into(),
            namepath: "script".into(),
            shebang: true,
            ..Recipe::default()
        }
    }

    fn executable(directory: &Path, name: &str, body: &str) -> std::path::PathBuf {
        let path = directory.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[test]
    fn optional_highlighter_accepts_only_sgr_and_times_out() {
        let _guard = ENVIRONMENT.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let valid = executable(
            directory.path(),
            "valid-bat",
            "printf '\\033[31mhighlighted\\033[0m\\n'",
        );
        unsafe { env::set_var("JTV_BAT", &valid) };
        assert_eq!(
            highlight_with_bat(&recipe(), "script:\n  echo safe\n").as_deref(),
            Some("\x1b[31mhighlighted\x1b[0m\n")
        );

        let hostile = executable(
            directory.path(),
            "hostile-bat",
            "printf '\\033]52;c;secret\\007'",
        );
        unsafe { env::set_var("JTV_BAT", &hostile) };
        assert!(highlight_with_bat(&recipe(), "safe").is_none());

        let oversized = executable(
            directory.path(),
            "oversized-bat",
            "dd if=/dev/zero bs=524289 count=1 2>/dev/null | tr '\\000' x",
        );
        unsafe { env::set_var("JTV_BAT", &oversized) };
        assert!(highlight_with_bat(&recipe(), "safe").is_none());

        let slow = executable(directory.path(), "slow-bat", "sleep 2");
        unsafe { env::set_var("JTV_BAT", &slow) };
        let started = Instant::now();
        assert!(highlight_with_bat(&recipe(), "safe").is_none());
        assert!(started.elapsed() < Duration::from_millis(1_500));
        unsafe { env::remove_var("JTV_BAT") };
    }
}
