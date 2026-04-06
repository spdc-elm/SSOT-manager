# SSOT-manager

Rust implementation workspace for the SSOT manager MVP.

- Scope: deterministic management of personal AI flavour assets as a single source of truth.
- Core responsibilities: validate config, resolve a named profile, plan changes, apply safe symlink updates, detect drift, and undo the last successful apply.
- Current design draft: [../draft.md](../draft.md)

Config uses a global `source_root` by default, and a profile may optionally override it with `profiles.<name>.source_root` when a subset of rules should resolve from a different source directory.

## MVP Commands

```bash
cargo run -- --config examples/personal-harness-management.yaml config validate
cargo run -- --config examples/personal-harness-management.yaml profile plan skill-global
cargo run -- --config examples/personal-harness-management.yaml profile apply skill-global
cargo run -- --config examples/personal-harness-management.yaml profile doctor skill-global
cargo run -- --config examples/personal-harness-management.yaml undo
```

Use `--state-dir <path>` if you want journals and managed records somewhere other than the default state directory.

## Safety Model

- The MVP accepts only `mode: symlink`.
- `profile apply` refuses any plan that contains `danger` actions.
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

[`examples/personal-harness-management.yaml`](examples/personal-harness-management.yaml) uses relative paths so it can be run from this repo checkout and syncs to `/tmp/ssot-manager-example/` so the workflow can be exercised without touching consumer config directories. The example also shows `kg-local` overriding the global `source_root`.

## Explicit Non-Goals

- No profile composition or include semantics yet
- No `copy` or `hardlink` support yet
- No transform hooks or template rendering
- No TUI or interactive config editing
