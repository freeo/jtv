# Configuration

Configuration is optional. `jtv` searches upward from its invocation directory for
`.jtv.toml`. Recipe keys use the complete `just` namepath, including modules.

```toml
[recipes.deploy.params.environment]
type = "choice"
values = ["development", "staging", "production"]

[recipes.deploy.params.manifest]
type = "file"

[recipes.deploy.params.confirm]
type = "boolean"

[recipes.login.params.token]
type = "secret"
```

Supported types are:

- `string`: ordinary terminal input; this is the default. `Tab` opens a recursive
  Television file/directory picker rooted at the current working directory and uses
  the current input as its query. Selection returns a relative path; cancellation
  preserves the unfinished input.
- `secret`: input with terminal echo disabled and output redaction.
- `choice`: a Television picker over the configured `values`.
- `boolean`: a Television true/false picker.
- `file`: a Television file picker rooted at the project directory.
- `directory`: a Television directory picker rooted at the project directory.

`secret` is also the exact boundary for optional shell-history integration. If
an executed command emits a parameter configured as secret, jtv adds no
synthetic zsh or Atuin entry for that command. It does not guess from parameter
names or values, and it does not add a `[REDACTED]` command. Sensitive parameters
must therefore be declared explicitly.

The `Tab` completion hook applies only to ordinary non-secret strings, including
each variadic string value. It does not replace the dedicated behavior of `secret`,
`choice`, `boolean`, `file`, or `directory` parameters.

Unknown recipe names, parameter names, types, or fields are errors so configuration
typos cannot silently alter execution.

For a workspace-wide unscoped launch, the `.jtv.toml` found from the startup
directory configures only the primary root project. Recursively discovered child
Justfiles currently use the safe default string prompting behavior. This prevents
a root recipe key from accidentally configuring an unrelated same-named child
recipe; per-child configuration/namespacing is intentionally deferred.
