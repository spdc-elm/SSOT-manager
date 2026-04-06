# SSOT-manager

Rust implementation workspace for the SSOT manager MVP.

- Scope: deterministic management of personal AI flavour assets as a single source of truth.
- Core responsibilities: validate config, compile generated prompt assets, resolve a named profile, plan changes, apply safe materialization updates, detect drift, and undo the last successful apply.
- Current design draft: [../draft.md](../draft.md)

Config uses a global `source_root` by default, and a profile may optionally override it with `profiles.<name>.source_root` when a subset of rules should resolve from a different source directory. Prompt compositions live under the shared `source_root` and emit generated files there before sync.

## MVP Commands

```bash
cargo run -- --config examples/personal-harness-management.yaml config validate
cargo run -- --config examples/personal-harness-management.yaml prompt list
cargo run -- --config examples/personal-harness-management.yaml prompt show codex-agent
cargo run -- --config examples/personal-harness-management.yaml prompt preview codex-agent
cargo run -- --config examples/personal-harness-management.yaml prompt build codex-agent
cargo run -- --config examples/personal-harness-management.yaml profile list
cargo run -- --config examples/personal-harness-management.yaml profile show skill-global
cargo run -- --config examples/personal-harness-management.yaml profile explain skill-global
cargo run -- --config examples/personal-harness-management.yaml profile plan skill-global
cargo run -- --config examples/personal-harness-management.yaml profile apply skill-global
cargo run -- --config examples/personal-harness-management.yaml profile doctor skill-global
cargo run -- --config examples/personal-harness-management.yaml tui
cargo run -- --config examples/personal-harness-management.yaml undo
```

Use `--state-dir <path>` if you want journals and managed records somewhere other than the default state directory.
The inspection commands also accept `--json` for machine-readable output.

## Safety Model

- Supported materialization modes are `symlink`, `copy`, and `hardlink`.
- Supported prompt rendering is intentionally narrow: built-in `concat` with optional per-input and outer wrappers plus declared variable interpolation.
- `profile apply` refuses any plan that contains `danger` actions.
- Profiles that declare `requires` will block plan/apply when a required composition output is missing or stale.
- Unmanaged files and directories are never overwritten silently.
- Managed records and the last successful apply journal are written only after filesystem verification succeeds.
- `undo` only touches targets that belong to the most recent successful apply journal.

## State Directory

Default state location:

- `$XDG_STATE_HOME/ssot-manager` when `XDG_STATE_HOME` is set
- otherwise `~/.local/state/ssot-manager`

Stored files:

- `managed-records.json`: current ownership records keyed by target path
- `last-apply.json`: the last successful apply journal used by `undo`

## Example Config

[`examples/personal-harness-management.yaml`](examples/personal-harness-management.yaml) uses relative paths so it can be run from this repo checkout and syncs to `/tmp/ssot-manager-example/` so the workflow can be exercised without touching consumer config directories. The example shows a `codex-agent` composition that compiles `Agents/assistant.md` and `USER.md` into `build/prompts/codex/AGENTS.generated.md`, and a profile that declares `requires: [codex-agent]` before syncing that generated file.

If you compile generated assets into your repo, keep the generated path gitignored. The example uses `build/prompts/` for that reason.

## Inspection Workflow

- `prompt list` shows configured prompt compositions and their output paths.
- `prompt show <name>` shows the effective recipe for a named composition.
- `prompt preview <name>` renders compiled prompt text without writing the generated file.
- `prompt build <name>` materializes the generated prompt file under `source_root`.
- `profile list` shows the configured profiles and their effective source roots.
- `profile show <name>` shows a profile's declared rules together with its required prompt compositions and their readiness.
- `profile explain <name>` combines prerequisite status, profile resolution diagnostics, and the current reconcile plan so later UI layers can consume the same explanation model.

## Thin TUI

- `tui` opens a profile-centered terminal UI backed by the same library inspection and reconcile logic as the CLI.
- Navigation: `Up`/`Down` or `j`/`k` changes the selected profile, `Tab`/`Left`/`Right` switches between `Show`, `Plan`, and `Doctor`.
- Actions: `c` compiles the selected profile's required prompt compositions, `a` applies the selected profile, `u` runs `undo`, `r` refreshes state, and `q` quits.

## Explicit Non-Goals

- No profile composition or include semantics yet
- No arbitrary script hooks or general workflow automation
- No separate composition-browser TUI or interactive config editing
