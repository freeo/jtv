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
jtv --justfile path/to/justfile
jtv --module docker
jtv --dry-run
jtv --color always --icons unicode
jtv --color never --icons ascii
```

Navigate and search using Television's normal controls. `Tab` toggles recipes in a
multi-selection and `Escape` cancels without executing anything. Selected recipes
run in deterministic recipe-name order and the queue stops on the first failure.
Television 0.15.9 exposes a multi-selection as an unordered set, so toggle order is
not available to integrations.

Recipe rows show parameter requirements/defaults, dependencies, groups, and a
bounded summary cue. The Details preview provides structured metadata and recipe
body; `Ctrl-F` cycles to the faithful `just --show` Definition preview. `Ctrl-X`
opens Television's action menu, including an explicit dry-run action.

`--color auto|always|never` and `--icons auto|unicode|ascii|none` are independent.
Auto mode honors `NO_COLOR`, `TERM=dumb`, and `NO_ICONS=1`; terminals below 100
columns use the compact portrait layout. Released Television 0.15.9 cannot safely
combine ANSI source rows with jtv's opaque callback IDs, so jtv automatically
uses plain source rows until a TV build passes the capability gate. All structural cues and icons remain;
the exact color capability is guarded by the upstream test described in the
[testing guide](docs/testing.md).

Ordinary arguments use a terminal prompt. Enumerable values and paths return to a
Television picker. Optional project metadata in `.jtv.toml` can declare those types;
see [configuration](docs/configuration.md).

`jtv` does not write `.just_history`, `.just-tv-last-command`, or parent-shell
history. Television's own frecency remains available.

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
