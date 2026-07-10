# Visual design

`jtv` uses Television for terminal layout, focus, fuzzy matching, selection, and
preview navigation. `jtv` owns the semantic presentation of Just recipes. The
same information must remain understandable with color and icons disabled.

## Semantic palette

The palette deliberately uses ANSI-16 roles so the user's terminal theme
controls the final contrast.

| Role | ANSI | Meaning | Plain-text cue |
|---|---:|---|---|
| Recipe | cyan (`36`) | Executable recipe name | Recipe name |
| Parameter | bold yellow (`1;33`) | Optional/default parameter name | `name:default` |
| Required | bright red (`91`) | Required input | `<required>` |
| Default | green (`32`) | Literal default value | Value after `:` or `=` |
| Dependency | bright magenta (`95`) | Prerequisite recipe | `→ dependency` |
| Module header | bold white (`1;37`) | Module context | `Module: name` |
| Docker module | bright blue (`94`) | `docker::` prefix | `docker::` |
| Test module | bright green (`92`) | `test::`/`testing::` prefix | Module prefix |
| Deploy module | bright yellow (`93`) | `deploy::`/`deployment::` prefix | Module prefix |
| Other module | bright cyan (`96`) | Any other module prefix | Module prefix |
| Documentation | magenta (`35`) | Recipe documentation | `Summary` section |
| Attribute | bright blue (`94`) | Just attributes and metadata | Labeled badge/row |
| Signature | cyan (`36`) | Just recipe signature | `Signature` section |
| Separator | dim (`2`) | Structural divider or secondary text | Section spacing/label |

Colors reinforce meaning; they never replace the plain-text cue.

## Icons

Unicode mode preserves the archived vocabulary:

- `▶` classic recipe in a non-modular Justfile
- `🔷` core recipe in a modular project
- `🐳` docker module
- `🧪` test/testing module
- `🚀` deploy/deployment module
- `📦` any other module

ASCII mode uses `[recipe]`, `[core]`, `[docker]`, `[test]`, `[deploy]`, and
`[mod]`. `none` removes the leading marker without removing module text.

## Recipe rows

Rows keep the archived spacing: one space after the icon and two spaces before
compact metadata. Core recipes precede module recipes when the query is empty.
Fuzzy matching may reorder results after the user types.

Examples shown without color:

```text
▶ build  target:<required> profile:debug → prepare
🐳 docker::publish  tag:latest
[test] test::unit  filter:<required>
```

Wide mode may include bounded metadata or a short documentation cue. Compact
mode retains icon/label, full namepath, and required/default markers; complete
details remain in the preview.

## Preview hierarchy

The default `Details` preview is ordered as follows:

1. module context and recipe title;
2. alias, group, quiet, and shebang badges when present;
3. summary/documentation;
4. parameters with required/default, flag spelling, cardinality, configured
   picker type, and help;
5. dependencies;
6. attributes;
7. recipe signature or compact definition.

The `Definition` preview shows faithful `just --show` output. Optional `bat`
highlighting may enhance a shebang body, but absence or failure must retain the
same text and must not produce an error. It is currently enabled on Unix only;
Windows uses the deterministic internal renderer until bounded process-tree
termination is implemented there.

## Prompts and execution plan

After Television closes, `jtv` prints one concise recipe or queue context.
Parameter prompts show `[current/total]`, immutable parameter identity, input
type, and required/default state. Secret values are never echoed.

The execution plan lists recipes in execution order using safe display quoting.
Secret values appear only as `[REDACTED]`. Confirmation remains mandatory; a
decline executes nothing.

## Accessibility and terminal modes

- `--color=auto` honors `NO_COLOR` and `TERM=dumb`; explicit `always` or
  `never` wins.
- `--icons=auto` honors legacy `NO_ICONS=1`; explicit `unicode`, `ascii`, or
  `none` wins.
- Plain mode contains no escape byte and preserves the same visible wording.
- ANSI styling uses only renderer-owned SGR sequences. Justfile/config content
  is sanitized before styles are applied.
- OSC hyperlinks/clipboard commands, cursor/erase commands, BEL, CR, C1
  controls, and bidi overrides are never forwarded.
- Narrow terminals use a compact/portrait presentation; 120 columns and wider
  use the landscape presentation unless the user toggles Television's layout.

Television 0.15.9 cannot yet preserve ANSI source styling together with a
separate display template and opaque callback output. jtv deliberately prefers
safe identity and readable plain rows for every unverified build. A custom/upstream build
may opt into the capability with `JTV_UNSAFE_TV_ANSI_DISPLAY=1`, but it must first pass
the real VT-cell gate documented in `docs/testing.md`; visible text alone is not
evidence that semantic colors survived.

## Testing contract

Renderer tests prove semantic roles and hostile-input safety. VT tests prove
cell colors/modifiers and reset behavior. Real Television snapshots record
visible text plus stable non-default style runs. Exact style snapshots are
Linux-only and pinned to the supported Television version; cross-platform tests
assert semantic behavior rather than pixel/font identity.
