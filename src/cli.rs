//! Public and hidden command-line entry points.

use std::{
    collections::BTreeSet,
    env,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use clap::{Parser, Subcommand};
use dialoguer::Confirm;
use semver::Version;

use crate::{
    Error, Result,
    channel::{self, InstallOutcome},
    command::build_plan,
    config::Config,
    invocation::Invocation,
    just,
    parameters::{DialoguerPrompter, collect, dialoguer_theme},
    picker::{PickerState, TvPicker},
    presentation::{
        ColorMode, IconMode, PresentationOptions, ResolvedColorMode, StyleRole, StyledText,
    },
    runner::{ProcessExecutor, run_queue},
    session::{CatalogTarget, SessionFile, SessionState, WorkspaceCatalog},
    target::{self, ResolvedTarget, TargetKind},
    television,
    workspace::{self, WorkspaceOrigin, WorkspaceWarning},
};

const MIN_JUST: Version = Version::new(1, 53, 0);

#[derive(Debug, Parser)]
#[command(
    name = "jtv",
    version,
    about = "Interactive Justfile runner powered by Television"
)]
struct Cli {
    /// Open a root module or named standalone Justfile collection.
    #[arg(value_name = "NAME", conflicts_with_all = ["justfile", "module"])]
    target: Option<String>,

    /// Use a specific Justfile instead of `just` discovery.
    #[arg(long, value_name = "PATH")]
    justfile: Option<PathBuf>,

    /// Show recipes below one complete module namepath.
    #[arg(long, value_name = "NAMEPATH")]
    module: Option<String>,

    /// Ask `just` to print commands without running them.
    #[arg(long)]
    dry_run: bool,

    /// Control semantic colors emitted inside Television and prompts.
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    color: ColorMode,

    /// Control recipe and module icons independently from color.
    #[arg(long, value_enum, default_value_t = IconMode::Auto)]
    icons: IconMode,

    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Install or update the bundled Television channel.
    Init {
        /// Back up and replace a modified existing channel.
        #[arg(long)]
        force: bool,
    },
    /// Check `just`, Television, and channel compatibility.
    Doctor,
    #[command(name = "__tv-source", hide = true)]
    TvSource {
        #[arg(long, value_enum, default_value_t = television::SourceView::Root)]
        view: television::SourceView,
    },
    #[command(name = "__tv-preview", hide = true)]
    TvPreview {
        #[arg(long)]
        definition: bool,
        id: String,
    },
    #[command(name = "__tv-run", hide = true)]
    TvRun {
        #[arg(long)]
        dry_run: bool,
        ids: Vec<String>,
    },
    #[command(name = "__picker-source", hide = true)]
    PickerSource,
}

pub fn run() -> Result<i32> {
    crate::cleanup::install_signal_handler()?;
    let cli = Cli::parse();
    match cli.command {
        Some(CliCommand::Init { force }) => init(force),
        Some(CliCommand::Doctor) => doctor(),
        Some(CliCommand::TvSource { view }) => tv_source(view),
        Some(CliCommand::TvPreview { definition, id }) => tv_preview(&id, definition),
        Some(CliCommand::TvRun { dry_run, ids }) => tv_run(&ids, dry_run),
        Some(CliCommand::PickerSource) => picker_source(),
        None => launch(
            cli.target,
            cli.justfile,
            cli.module,
            cli.dry_run,
            cli.color,
            cli.icons,
        ),
    }
}

fn invocation(
    justfile: Option<PathBuf>,
    module_filter: Option<String>,
    dry_run: bool,
) -> Result<Invocation> {
    Invocation::new(
        env::current_dir().map_err(|source| Error::Read {
            path: PathBuf::from("."),
            source,
        })?,
        justfile,
        module_filter,
        dry_run,
    )
    .canonicalized()
}

fn init(force: bool) -> Result<i32> {
    let cable_dir = channel::default_cable_dir()?;
    match channel::install(&cable_dir, force)? {
        InstallOutcome::Installed { path } => println!("installed {}", path.display()),
        InstallOutcome::AlreadyCurrent { path } => {
            println!("already current: {}", path.display())
        }
        InstallOutcome::Replaced { path, backup } => {
            println!("replaced {} (backup: {})", path.display(), backup.display())
        }
    }
    Ok(0)
}

fn doctor() -> Result<i32> {
    let invocation = invocation(None, None, false)?;
    let mut healthy = true;

    match just_version(&invocation.just_binary) {
        Ok(version) if version >= MIN_JUST => {
            println!("[OK] just {version}");
            match just_contract(&invocation.just_binary) {
                Ok(()) => println!("[OK] just JSON contract"),
                Err(error) => {
                    println!("[FAIL] just JSON contract: {error}");
                    healthy = false;
                }
            }
        }
        Ok(version) => {
            println!("[FAIL] just {version}; jtv requires {MIN_JUST} or newer");
            healthy = false;
        }
        Err(error) => {
            println!("[FAIL] just: {error}");
            healthy = false;
        }
    }

    match television::version(&invocation.tv_binary) {
        Ok(version) if television::version_is_supported(&version) => {
            println!("[OK] television {version}");
            println!(
                "[WARN] recipe-row colors remain disabled until a Television ANSI+display release passes jtv's VT-cell gate"
            );
        }
        Ok(version) => {
            println!("[FAIL] television {version}; jtv requires 0.15.9 or newer");
            healthy = false;
        }
        Err(error) => {
            println!("[FAIL] television: {error}");
            healthy = false;
        }
    }

    match channel::default_cable_dir() {
        Ok(path) => match channel::channel_is_current(&path) {
            Ok(true) => println!("[OK] channel {}", channel::channel_path(&path).display()),
            Ok(false) => {
                println!("[FAIL] jtv-recipes channel is missing or outdated; run `jtv init`");
                healthy = false;
            }
            Err(error) => {
                println!("[FAIL] channel: {error}");
                healthy = false;
            }
        },
        Err(error) => {
            println!("[FAIL] Television configuration: {error}");
            healthy = false;
        }
    }

    Ok(if healthy { 0 } else { 1 })
}

fn launch(
    target_name: Option<String>,
    justfile: Option<PathBuf>,
    module_filter: Option<String>,
    dry_run: bool,
    color: ColorMode,
    icons: IconMode,
) -> Result<i32> {
    let cwd = env::current_dir().map_err(|source| Error::Read {
        path: PathBuf::from("."),
        source,
    })?;
    let resolved = if let Some(name) = target_name {
        let root_project = if target::has_discoverable_root(&cwd) {
            Some(just::load_project(&invocation(None, None, dry_run)?)?)
        } else {
            None
        };
        Some(target::resolve(&cwd, &name, root_project.as_ref())?)
    } else {
        None
    };
    let (justfile, module_filter) = match resolved.as_ref().map(|target| &target.selected) {
        Some(TargetKind::Module(module)) => (None, Some(module.clone())),
        Some(TargetKind::Justfile(path)) => (Some(path.clone()), None),
        None => (justfile, module_filter),
    };
    let scoped = resolved.is_some() || justfile.is_some() || module_filter.is_some();
    let invocation = invocation(justfile, module_filter, dry_run)?;
    ensure_runtime_compatible(&invocation)?;
    let primary_dump = just::load_project_dump(&invocation)?;
    let loaded = validate_config(&invocation, &primary_dump.project)?;
    let cable_dir = channel::default_cable_dir()?;
    if !channel::channel_is_current(&cable_dir)? {
        return Err(Error::Message(
            "the jtv Television channel is missing or outdated; run `jtv init`".into(),
        ));
    }
    let columns = if io::stderr().is_terminal() {
        let (_, columns) = console::Term::stderr().size();
        (columns > 0).then_some(columns)
    } else {
        None
    };
    let mut presentation = PresentationOptions::resolve(color, icons, columns);
    if presentation.color == ResolvedColorMode::Color
        && env::var_os("JTV_UNSAFE_TV_ANSI_DISPLAY").as_deref() != Some(std::ffi::OsStr::new("1"))
    {
        // TV 0.15.9 renders source SGR bytes visibly when `ansi` and `display`
        // are combined. Preserve opaque IDs and readable rows for every build
        // until that exact build passes the opt-in VT-cell capability gate.
        presentation.source_color = ResolvedColorMode::Plain;
    }
    let catalog = if scoped {
        WorkspaceCatalog::from_primary(invocation.clone(), primary_dump.project, loaded.config)
    } else {
        build_workspace_catalog(invocation.clone(), primary_dump, loaded.config)?
    };
    if catalog
        .targets
        .iter()
        .all(|target| target.project.recipes.is_empty())
    {
        return Err(Error::Message("the workspace has no public recipes".into()));
    }
    let state = SessionState::new_with_catalog(catalog, presentation)?;
    let session = SessionFile::create(&state)?;
    let status = television::launch(
        &invocation.tv_binary,
        &cable_dir,
        &invocation.cwd,
        &session,
        &state.presentation,
    )?;
    print_overlap_warning(resolved.as_ref())?;
    print_workspace_warnings(&state.catalog.warnings)?;
    Ok(status.code().unwrap_or(130))
}

fn build_workspace_catalog(
    invocation: Invocation,
    primary_dump: just::ProjectDump,
    config: Config,
) -> Result<WorkspaceCatalog> {
    let primary_source = primary_dump
        .source
        .as_deref()
        .or(invocation.justfile.as_deref())
        .map(Path::to_path_buf)
        .or_else(|| target::discoverable_root(&invocation.cwd));
    let Some(primary_source) = primary_source else {
        // `source` is additive JSON metadata. Keep compatible test doubles and
        // older producers useful, but restrict them to the primary catalog
        // because recursive exclusion cannot be proven without its path.
        return Ok(WorkspaceCatalog::from_primary(
            invocation,
            primary_dump.project,
            config,
        ));
    };
    let mut primary_invocation = invocation.clone();
    primary_invocation.justfile = Some(primary_source.clone());
    let discovery = workspace::discover(
        &invocation.cwd,
        &primary_source,
        &primary_dump.module_sources,
    );
    let mut warnings = discovery.warnings;
    // Parsing additional targets dominates workspace startup. Bound the worker
    // count so large monorepos cannot create one process/thread per Justfile,
    // then restore discovery order before building IDs and warnings.
    let worker_count = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(4)
        .min(discovery.justfiles.len().max(1));
    let mut batches = (0..worker_count).map(|_| Vec::new()).collect::<Vec<_>>();
    for (index, justfile) in discovery.justfiles.into_iter().enumerate() {
        batches[index % worker_count].push((index, justfile));
    }
    let mut results = std::thread::scope(|scope| {
        let handles = batches.into_iter().map(|batch| {
            let invocation = &invocation;
            scope.spawn(move || {
                batch
                    .into_iter()
                    .map(|(index, justfile)| {
                        let mut child_invocation = invocation.clone();
                        child_invocation.justfile = Some(justfile.path.clone());
                        child_invocation.module_filter = None;
                        let result = just::load_project_dump(&child_invocation)
                            .map_err(|error| error.to_string());
                        (index, justfile, child_invocation, result)
                    })
                    .collect::<Vec<_>>()
            })
        });
        handles
            .flat_map(|handle| handle.join().expect("workspace loader worker panicked"))
            .collect::<Vec<_>>()
    });
    results.sort_by_key(|(index, _, _, _)| *index);
    let mut loaded = Vec::new();
    let mut failed = Vec::new();
    for (_, justfile, child_invocation, result) in results {
        match result {
            Ok(dump) => loaded.push((justfile, child_invocation, dump)),
            Err(error) => failed.push((justfile, error)),
        }
    }

    let module_sources = loaded
        .iter()
        .flat_map(|(_, _, dump)| dump.module_sources.iter())
        .filter_map(|source| source.canonicalize().ok())
        .collect::<BTreeSet<_>>();
    for (justfile, message) in failed {
        if !module_sources.contains(&justfile.path) {
            warnings.push(WorkspaceWarning {
                path: justfile.relative_path,
                message,
            });
        }
    }

    let mut targets = vec![CatalogTarget {
        origin: WorkspaceOrigin::Root,
        invocation: primary_invocation,
        project: primary_dump.project,
        config,
    }];
    targets.extend(
        loaded
            .into_iter()
            .filter(|(justfile, _, _)| !module_sources.contains(&justfile.path))
            .map(|(justfile, invocation, dump)| CatalogTarget {
                origin: WorkspaceOrigin::Subfolder {
                    relative_justfile: justfile.relative_path,
                    label: justfile.label,
                },
                invocation,
                project: dump.project,
                config: Config::default(),
            }),
    );
    warnings.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.message.cmp(&right.message))
    });
    Ok(WorkspaceCatalog::new(invocation.cwd, targets, warnings))
}

fn print_overlap_warning(target: Option<&ResolvedTarget>) -> Result<()> {
    if let Some(warning) = target.and_then(ResolvedTarget::warning) {
        println!("{warning}");
        io::stdout().flush().map_err(|source| Error::Write {
            path: PathBuf::from("<stdout>"),
            source,
        })?;
    }
    Ok(())
}

fn print_workspace_warnings(warnings: &[WorkspaceWarning]) -> Result<()> {
    if warnings.is_empty() {
        return Ok(());
    }
    println!("WARNING: some workspace Justfiles were skipped:");
    for warning in warnings {
        println!("  {}: {}", warning.path.display(), warning.message);
    }
    io::stdout().flush().map_err(|source| Error::Write {
        path: PathBuf::from("<stdout>"),
        source,
    })
}

fn tv_source(view: television::SourceView) -> Result<i32> {
    let state = crate::session::load_from_env()?;
    print!("{}", television::source_output(&state, view)?);
    io::stdout().flush().map_err(|source| Error::Write {
        path: PathBuf::from("<stdout>"),
        source,
    })?;
    Ok(0)
}

fn tv_preview(id: &str, definition: bool) -> Result<i32> {
    let state = crate::session::load_from_env()?;
    let preview = if definition {
        television::definition_preview(&state, id)?
    } else {
        television::preview(&state, id)?
    };
    print!("{preview}");
    Ok(0)
}

fn tv_run(ids: &[String], dry_run: bool) -> Result<i32> {
    if ids.is_empty() {
        return Err(Error::Message(
            "Television supplied no recipe selections".into(),
        ));
    }
    let state = crate::session::load_from_env()?;
    let selected_ids = ids.iter().map(String::as_str).collect::<BTreeSet<_>>();
    for id in &selected_ids {
        state.resolve(id)?;
    }
    let recipes = state
        .catalog
        .selections
        .keys()
        .filter(|id| selected_ids.contains(id.as_str()))
        .map(|id| state.resolve(id))
        .collect::<Result<Vec<_>>>()?;

    let mut prompts = DialoguerPrompter::new(state.presentation.color);
    let mut picker = TvPicker::new(state.primary_target()?.invocation.tv_binary.clone())?;
    let mut plans = Vec::with_capacity(recipes.len());
    let mut context = StyledText::new();
    if recipes.len() == 1 {
        context
            .inline("Recipe: ", StyleRole::Dim)
            .inline(selection_label(&recipes[0]), StyleRole::Recipe)
            .newline();
    } else {
        context.inline(
            format!("Queue ({} recipes)\n", recipes.len()),
            StyleRole::Bold,
        );
        for selected in &recipes {
            context
                .inline("• ", StyleRole::Plain)
                .inline(selection_label(selected), StyleRole::Recipe)
                .newline();
        }
    }
    eprint!("{}", context.render(state.presentation.color));
    for selected in recipes {
        let values = collect(
            selected.recipe,
            &selected.target.config,
            &state.catalog.root,
            &mut prompts,
            &mut picker,
        )?;
        let mut invocation = selected.target.invocation.clone();
        invocation.dry_run |= dry_run;
        plans.push(build_plan(&invocation, selected.recipe, &values)?);
    }

    let mut heading = StyledText::new();
    heading.inline("Execution plan:", StyleRole::Bold);
    eprintln!("{}", heading.render(state.presentation.color));
    for plan in &plans {
        eprintln!("  {}", plan.display_redacted());
    }
    let theme = dialoguer_theme(state.presentation.color);
    let confirmed = Confirm::with_theme(theme.as_ref())
        .with_prompt("Run selected recipe(s)?")
        .default(true)
        .interact()
        .map_err(|error| Error::Message(format!("confirmation failed: {error}")))?;
    if !confirmed {
        return Ok(0);
    }
    run_queue(&plans, &mut ProcessExecutor)
}

fn selection_label(selected: &crate::session::ResolvedSelection<'_>) -> String {
    match &selected.target.origin {
        WorkspaceOrigin::Root => selected.recipe.namepath.clone(),
        WorkspaceOrigin::Subfolder { label, .. } => {
            format!("{label}  {}", selected.recipe.namepath)
        }
    }
}

fn picker_source() -> Result<i32> {
    println!("{}", PickerState::load_from_env()?.source_output());
    Ok(0)
}

fn validate_config(
    invocation: &Invocation,
    project: &crate::model::Project,
) -> Result<crate::config::LoadedConfig> {
    let loaded = Config::load_upward(&invocation.cwd)?;
    loaded.config.validate(
        project,
        loaded.path.as_deref().unwrap_or(Path::new(".jtv.toml")),
    )?;
    Ok(loaded)
}

fn just_version(binary: &Path) -> Result<Version> {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .map_err(|source| Error::Spawn {
            program: binary.display().to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(Error::ProgramFailed {
            program: binary.display().to_string(),
            status: output.status.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let value = text
        .split_whitespace()
        .find(|word| word.as_bytes().first().is_some_and(u8::is_ascii_digit))
        .ok_or_else(|| Error::Message(format!("unrecognized just version output: {text}")))?;
    Version::parse(value)
        .map_err(|error| Error::Message(format!("unrecognized just version `{value}`: {error}")))
}

fn just_contract(binary: &Path) -> Result<()> {
    let mut child = Command::new(binary)
        .args(["--justfile", "-", "--dump", "--dump-format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| Error::Spawn {
            program: binary.display().to_string(),
            source,
        })?;
    child
        .stdin
        .as_mut()
        .expect("piped stdin is available")
        .write_all(b"probe:\n    @:\n")
        .map_err(|source| Error::Write {
            path: PathBuf::from("<just stdin>"),
            source,
        })?;
    let output = child.wait_with_output().map_err(|source| Error::Spawn {
        program: binary.display().to_string(),
        source,
    })?;
    if !output.status.success() {
        return Err(Error::ProgramFailed {
            program: binary.display().to_string(),
            status: output.status.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let project = just::parse_project(&output.stdout, None)?;
    if project.recipe("probe").is_none() {
        return Err(Error::Message(
            "JSON dump did not contain the compatibility probe recipe".into(),
        ));
    }
    Ok(())
}

fn ensure_runtime_compatible(invocation: &Invocation) -> Result<()> {
    let just = just_version(&invocation.just_binary)?;
    if just < MIN_JUST {
        return Err(Error::Message(format!(
            "just {just} is unsupported; install {MIN_JUST} or newer"
        )));
    }
    just_contract(&invocation.just_binary).map_err(|error| {
        Error::Message(format!(
            "just {} does not provide the required JSON contract: {error}",
            invocation.just_binary.display()
        ))
    })?;
    let tv = television::version(&invocation.tv_binary)?;
    if !television::version_is_supported(&tv) {
        return Err(Error::Message(format!(
            "television {tv} is unsupported; install 0.15.9 or newer"
        )));
    }
    Ok(())
}
