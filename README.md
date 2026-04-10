# SSOT-manager

Rust implementation workspace for the SSOT manager MVP.

- Scope: deterministic management of personal AI flavour assets as a single source of truth.
- Core responsibilities: validate config, compile generated prompt assets, resolve a named profile, plan changes, apply safe materialization updates, detect drift, and undo the last successful apply.
- Current design draft: [draft.md](draft.md)

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
cargo run -- --config examples/personal-harness-management.yaml profile apply skill-global --force-with-backup
cargo run -- --config examples/personal-harness-management.yaml profile doctor skill-global
cargo run -- --config examples/personal-harness-management.yaml tui
cargo run -- --config examples/personal-harness-management.yaml undo
```

Use `--state-dir <path>` if you want journals and managed records somewhere other than the default state directory.
The inspection commands also accept `--json` for machine-readable output.

## Install

Download a published release asset manually, or install on Unix-like systems with:

```bash
curl -fsSL https://raw.githubusercontent.com/spdc-elm/SSOT-manager/main/scripts/install.sh | sh
```

Useful variants:

```bash
curl -fsSL https://raw.githubusercontent.com/spdc-elm/SSOT-manager/main/scripts/install.sh | sh -s -- --version v0.1.0
curl -fsSL https://raw.githubusercontent.com/spdc-elm/SSOT-manager/main/scripts/install.sh | sh -s -- --install-dir /usr/local/bin
```

The installer currently supports the published Unix release targets:

- Linux x86_64
- macOS x86_64
- macOS aarch64

By default it tries to reuse an existing `ssot-manager` location first. Otherwise it prefers a writable common bin directory that is already on `PATH`, such as `/usr/local/bin`, `/opt/homebrew/bin`, `~/.local/bin`, or `~/bin`. If none match, it falls back to `~/.local/bin`.

The installed executable and CLI help examples use `ssot-manager`. Do not assume an `ssot` binary exists on `PATH`.

## Update

There is no separate updater yet. Updating is the same operation as installing again:

```bash
curl -fsSL https://raw.githubusercontent.com/spdc-elm/SSOT-manager/main/scripts/install.sh | sh
```

That fetches the latest GitHub Release and replaces the existing binary in place. To pin or roll back, rerun the installer with `--version <tag>`.

## Path Resolution

- Relative `source_root` values are resolved relative to the config file directory, not the shell's current working directory.
- Relative `profiles.<name>.source_root` values follow the same rule.
- Relative destination paths in `to` are also resolved relative to the config file directory.
- `~/` expands from `HOME`.

## Safety Model

- Supported materialization modes are `symlink`, `copy`, and `hardlink`.
- Supported prompt rendering is intentionally narrow: built-in `concat` with optional per-input and outer wrappers plus declared variable interpolation.
- `profile apply` refuses any plan that contains `danger` actions.
- `profile apply --force-with-backup` may replace unmanaged file, directory, or symlink collisions only when every danger in the plan is forceable.
- Profiles that declare `requires` will block plan/apply when a required composition output is missing or stale.
- Unmanaged files and directories are never overwritten silently.
- Forced backup-overwrite actions store a restorable backup in the manager state directory so `undo` can restore the replaced unmanaged content.
- Managed records and the last successful apply journal are written only after filesystem verification succeeds.
- `undo` only touches targets that belong to the most recent successful apply journal.

## State Directory

Default state location:

- `$XDG_STATE_HOME/ssot-manager` when `XDG_STATE_HOME` is set
- otherwise `~/.local/state/ssot-manager`

Stored files:

- `managed-records.json`: current ownership records keyed by target path
- `last-apply.json`: the last successful apply journal used by `undo`
- `backups/`: manager-owned restore artifacts for the current force-with-backup journal, when present

## Example Config

[`examples/personal-harness-management.yaml`](examples/personal-harness-management.yaml) assumes the asset repo lives next to this repo at `../personal-harness-management/` and syncs to `/tmp/ssot-manager-example/` so the workflow can be exercised without touching consumer config directories. The example shows a `codex-agent` composition that compiles `Agents/assistant.md` and `USER.md` from that asset repo into `build/prompts/codex/AGENTS.generated.md`, and a profile that declares `requires: [codex-agent]` before syncing that generated file.

When authoring real configs, prefer explicit flat YAML over wildcard-heavy bundles when independent per-asset toggles matter. A good default is source-assets-first authoring: one applyable profile for a source bundle such as `skill-global`, one rule per source asset, and multiple `to` destinations on that rule when the same asset should sync to several consumers.

If you compile generated assets into your repo, keep the generated path gitignored. The example uses `build/prompts/` for that reason.

## Inspection Workflow

- `prompt list` shows configured prompt compositions and their output paths.
- `prompt show <name>` shows the effective recipe for a named composition.
- `prompt preview <name>` renders compiled prompt text without writing the generated file.
- `prompt build <name>` materializes the generated prompt file under `source_root`.
- `profile list` shows the configured profiles and their effective source roots.
- `profile show <name>` shows a profile's declared rules together with its required prompt compositions and their readiness.
- `profile explain <name>` combines prerequisite status, profile resolution diagnostics, and the current reconcile plan so later UI layers can consume the same explanation model.
- Forceable dangers are shown as `danger*` in CLI plan output so they remain dangerous by default but are distinguishable from non-forceable blockers.

## Thin TUI

- `tui` opens a profile-centered terminal UI backed by the same library inspection and reconcile logic as the CLI.
- Main shell modes: the TUI starts in profile-browse mode, where `Up`/`Down` or `j`/`k` changes the selected profile and the right pane stays as a live preview. Press `Enter` to focus the current detail pane for deeper reading, then use `Esc` to return to profile browsing without losing the selected profile or active tab.
- Detail reading: while the detail pane is focused, `Up`/`Down` or `j`/`k` scrolls the right-hand content instead of changing profiles. `PageUp`/`PageDown`/`Home`/`End` still provide larger scrolling jumps, and long detail content now shows both a visible position indicator in the detail pane title and a scrollbar gutter.
- View switching: `Tab`, `Left`, `Right`, and `l` keep switching between `Show`, `Plan`, and `Doctor`. In browse mode, `h` still moves to the previous tab; in detail focus, `h` first moves to the previous tab and only leaves detail focus when the current tab is already `Show`.
- Inspection actions: `c` compiles the selected profile's required prompt compositions, `a` applies the selected profile, `u` runs `undo`, `r` refreshes state, and `q` quits.
- Profile editing: `e` edits the selected profile, `n` creates a new profile, and `d` starts a delete confirmation for the selected profile.
- Inside the profile editor: `j`/`k` moves between fields, `Enter` edits the selected field or opens a focused collection editor, `s` saves, and `Esc` backs out or opens an unsaved-changes confirmation.
- Collection editors support add/edit/remove and in-place reordering with `J`/`K` for ordered items such as `requires`, rules, rule destinations, and rule tags, and long lists keep the selected entry visible inside the popup viewport with the same scrollbar gutter treatment used by the inspect pane.
- If the current profile plan contains only forceable dangers, the first `a` arms backup-overwrite confirmation and the second `a` executes the forced apply.
- Saving from the TUI rewrites the YAML config in normalized form. Comments and original formatting are not preserved after a TUI save.

## Explicit Non-Goals

- No profile composition or include semantics yet
- No arbitrary script hooks or general workflow automation
- No separate composition-browser TUI
