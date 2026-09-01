# jtv

`jtv` is an interactive Justfile runner powered by
[Television](https://github.com/alexpasmantier/television). Television owns recipe
search, previews, keybindings, and multi-selection; `jtv` reads `just`'s JSON model,
collects recipe arguments, and executes `just` without reconstructing shell commands.

The Rust application is the supported implementation. The scripts under
`jtv-0.3.0/` are archived prototypes and are not used by the application or tests.

## Requirements

- Rust 1.85 or newer when building from source
- `just` 1.53.0 or newer
- Television (`tv`) 0.15.9 or newer

## Install

GitHub Releases provide a static Linux x86_64 binary and a universal macOS
binary. Install the latest release without `sudo` to `~/.local/bin` with:

```sh
curl -fsSL https://github.com/freeo/jtv/releases/latest/download/jtv-install.sh | sh
```

The installer verifies the release SHA-256 checksum before installing. It still
needs `just` and Television at runtime. To install a particular release, use
that release's installer and set its version:

```sh
curl -fsSL https://github.com/freeo/jtv/releases/download/v0.4.0/jtv-install.sh | JTV_VERSION=0.4.0 sh
```

To remove it:

```sh
curl -fsSL https://github.com/freeo/jtv/releases/latest/download/jtv-install.sh | sh -s uninstall
```

Set `JTV_INSTALL_DIR` before `sh` to use another directory. Every release asset
also has a GitHub Actions provenance attestation; after downloading an asset,
verify it with `gh attestation verify ./jtv -R freeo/jtv`.

## Build and initialize

```sh
cargo build --release
install -m 0755 target/release/jtv ~/.local/bin/jtv
jtv init
jtv doctor
```

`jtv init` installs the bundled `jtv-recipes` cable channel into Television's
configuration directory. It refuses to replace a modified channel unless `--force`
is given; a forced replacement creates a backup.

## Use

```sh
jtv
jtv docker
jtv --justfile path/to/justfile
jtv --module docker
jtv --dry-run
jtv --color always --icons unicode
jtv --color never --icons ascii
```

`jtv NAME` first opens a matching public root module. Otherwise it uses the
first existing standalone target in this exact order: `NAME.just`,
`NAME/justfile`, `justfiles/NAME.just`, `justfiles/NAME/justfile`. No other
filename aliases are inferred. If several targets exist, the first still wins
and jtv prints every match to stdout after Television exits. Use `--justfile`
or `--module` when you want to select one explicitly; those options cannot be
combined with `NAME`.

Navigate and search using Television's normal controls. `Tab` toggles recipes in a
multi-selection and `Escape` cancels without executing anything. Selected recipes
run in deterministic recipe-name order and the queue stops on the first failure.
Television 0.15.9 exposes a multi-selection as an unordered set, so toggle order is
not available to integrations.

An unscoped `jtv` discovers the root project once, then recursively catalogs
additional `justfile`, `.justfile`, and exact lowercase `*.just` files below the
startup directory. It respects Git ignore rules, skips dependency/build trees,
and never follows symlinks. Press `Ctrl-S` to cycle **Root → Subfolders → Modules
→ All**. Root-level named files such as `docker.just` belong to Subfolders too;
explicit `jtv NAME`, `--justfile`, and `--module` launches remain focused rather
than expanding the workspace.

Subfolder rows use `📁` (or `[dir]` in ASCII mode) and retain their relative
origin, for example `📁 supabase/  migrate` and `📁 db/seed.just  reset`.
Recipes execute through their owning absolute Justfile, so `supabase/justfile`
runs with normal `just` semantics in `supabase/`; jtv and the invoking shell stay
in the directory where jtv started. Invalid child files are skipped and reported
as one warning block after Television exits; an invalid primary Justfile remains
fatal.

Recipe rows stay concise: recipe names, parameter requirements/defaults, and
dependencies provide the visual scan cues without summaries, groups, or arrows.
The Details preview provides structured metadata and recipe body; `Ctrl-F` cycles
to the faithful `just --show` Definition preview. `Ctrl-X` opens Television's
action menu, including an explicit dry-run action.

`--color auto|always|never` and `--icons auto|unicode|ascii|none` are independent.
Auto mode honors `NO_COLOR`, `TERM=dumb`, and `NO_ICONS=1`; terminals below 100
columns use the compact portrait layout. Released Television 0.15.9 cannot safely
combine ANSI source rows with jtv's opaque callback IDs, so jtv automatically
uses plain source rows until a TV build passes the capability gate. All structural cues and icons remain;
the exact color capability is guarded by the upstream test described in the
[testing guide](docs/testing.md).

Ordinary arguments use a terminal prompt. Press `Tab` there to open a recursive
Television picker rooted at the current working directory; the text already typed
becomes its search query. Selecting replaces the prompt buffer with a relative path,
while `Escape` returns with the text and cursor unchanged. Enumerable values and
explicitly typed paths use their dedicated Television pickers. Optional project
metadata in `.jtv.toml` can declare those types; see
[configuration](docs/configuration.md).

Standalone `jtv` does not write `.just_history`, `.just-tv-last-command`, or
parent-shell history. Television's own frecency remains available.

### Optional zsh history integration

To make commands executed through jtv available to zsh recall,
zsh-autosuggestions, and Atuin, add this explicit line to `.zshrc`:

```zsh
eval "$(jtv shell-init zsh)"
```

jtv never edits `.zshrc`. Without this wrapper, behavior is unchanged. With it,
history remains additive:

```text
jtv
just --justfile /project/justfile deploy
```

Multi-selection adds one entry per command actually attempted, in jtv's
deterministic execution order (Television 0.15.9 does not expose mark order).
Successful, failed, and dry-run attempts are recorded; declined, cancelled, and
unreached commands are not. When Atuin is active, matching records include the
real cwd, exit status, and duration. Commands that emit a parameter explicitly
configured as `type = "secret"` are silently omitted from both histories—there
is no guessed-secret detection and no redacted placeholder. Remove the eval line
to disable the integration.

## Development

```sh
just fmt
just lint
just test-fast
just test-contract
just test-tui
just test-snapshots
just test-tv-ansi-display # candidate upstream/custom TV build only
just test-all
```

See the [testing guide](docs/testing.md), the [architecture](docs/architecture.md), and the
[implementation plan](docs/plans/jtv-rust-application.md) for the contracts and
verification strategy.
