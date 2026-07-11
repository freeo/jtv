# Architecture

`jtv` has three process boundaries:

1. `just --dump --dump-format json` supplies recipe semantics.
2. Television's cable-channel interface supplies search, preview, selection, and
   actions.
3. `just` receives a direct operating-system argument vector for execution.

For an unscoped launch, the top-level process discovers additional `justfile`,
`.justfile`, and lowercase `*.just` targets once and loads their JSON models with
a bounded worker pool. It creates a private temporary session containing a
workspace catalog: startup root, independently invokable targets, normalized
recipe models, target-local configuration, warnings, and opaque selection IDs.
Source cycling filters this cached catalog; it performs no filesystem walk or
`just` call.

Television source, preview, and action helpers receive only the session location
through the environment and validated, session-bound ASCII IDs through channel
templates. Each ID resolves to both its recipe and owning invocation, so identical
namepaths in different Justfiles cannot collide. Recipe names, paths, display
text, and parameter values are never interpolated into channel templates.

The installed channel is a public integration artifact. Hidden `__tv-*` commands
and the `JTV_SESSION` environment variable are private implementation details and
may change between releases.

Parameter values are retained as `OsString` values through planning and execution.
A separately rendered, redacted representation is used for confirmation and
diagnostics; it is never executed.

Child execution passes an absolute `--justfile` as an OS argument and leaves the
jtv process cwd at the startup root. `just` therefore applies its own documented
Justfile-directory semantics without a shell `cd` or process-wide cwd mutation.
