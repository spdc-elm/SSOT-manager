## Why

The current draft is directionally sharp but still too broad to implement and validate quickly. We need an MVP that proves a smaller claim first: a personal SSOT manager can make prompt and skill asset sync visible, deterministic, and safe enough that users trust it more than ad hoc `ln -s` scripts.

## What Changes

- Introduce an MVP CLI centered on one YAML config file, one source root, and explicit named profiles for personal AI flavour assets.
- Limit the first implementation to deterministic, mechanical sync behavior for selected assets, with `plan`, `apply`, `doctor`, and `undo` workflows.
- Make the managed boundary explicit by recording what the tool owns and surfacing unmanaged collisions or dangerous overwrites before mutation.
- Support local verification and last-apply journaling so the tool can explain current state and recover from common mistakes.
- Defer profile composition, `hardlink`/`copy` modes, transform hooks, and any TUI work until the core sync loop is proven.

## Capabilities

### New Capabilities
- `profile-config-resolution`: Load and validate a single YAML config, resolve a named profile into concrete sync rules, and discover matching source assets deterministically.
- `sync-reconciliation`: Compute desired versus actual target state, show a dry-run plan, and apply conservative sync changes while blocking unsafe unmanaged overwrites by default.
- `managed-state-and-doctor`: Persist managed records and the last successful apply journal, detect broken or stale managed links, and support doctor and undo for recorded changes.

### Modified Capabilities
- None.

## Impact

- New SSOT manager implementation under the local `SSOT-manager/` workspace.
- New CLI surface for profile inspection, planning, applying, and recovery.
- New spec coverage for config resolution, sync reconciliation, and managed-state behavior.
- Local state storage for managed records and apply journals, likely under `~/.local/state/ssot-manager/`.
