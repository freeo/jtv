# Architecture

`jtv` has three process boundaries:

1. `just --dump --dump-format json` supplies recipe semantics.
2. Television's cable-channel interface supplies search, preview, selection, and
   actions.
3. `just` receives a direct operating-system argument vector for execution.

The top-level `jtv` process creates a private temporary session containing a
normalized recipe model and opaque selection-ID map. Television source, preview,
and action helpers receive only the session location through the environment and
validated ASCII IDs through channel templates. Recipe names, paths, display text,
and parameter values are never interpolated into those templates.

The installed channel is a public integration artifact. Hidden `__tv-*` commands
and the `JTV_SESSION` environment variable are private implementation details and
may change between releases.

Parameter values are retained as `OsString` values through planning and execution.
A separately rendered, redacted representation is used for confirmation and
diagnostics; it is never executed.

