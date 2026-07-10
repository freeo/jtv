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

- `string`: ordinary terminal input; this is the default.
- `secret`: input with terminal echo disabled and output redaction.
- `choice`: a Television picker over the configured `values`.
- `boolean`: a Television true/false picker.
- `file`: a Television file picker rooted at the project directory.
- `directory`: a Television directory picker rooted at the project directory.

Unknown recipe names, parameter names, types, or fields are errors so configuration
typos cannot silently alter execution.

