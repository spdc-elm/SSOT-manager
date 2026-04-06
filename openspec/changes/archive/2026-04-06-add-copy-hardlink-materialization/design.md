## Context

The current SSOT manager treats every resolved intent as a symlink operation. That assumption is embedded in config validation, plan classification, apply verification, doctor drift detection, and undo journal validation. As a result, simply accepting new rule values would be unsafe: the engine would still record copy and hardlink targets as if they were symlinks, and undo could not prove whether a concrete file target had been edited after apply.

The change is cross-cutting because the same materialization mode must remain consistent from YAML parsing through inspection output, reconcile execution, state persistence, doctor, and undo.

## Goals / Non-Goals

**Goals:**
- Accept `symlink`, `copy`, and `hardlink` as first-class materialization modes in config and resolved intents.
- Apply file and directory-tree targets according to the declared mode.
- Verify and record enough post-apply state for doctor and undo to work safely across all supported modes.
- Preserve existing danger blocking and managed ownership semantics.

**Non-Goals:**
- Introducing force-overwrite behavior for unmanaged collisions.
- Supporting special filesystem node types beyond regular files, directories, and symlinks already encountered in the source tree.
- Adding a new state backend or changing the CLI surface beyond exposing the new modes through existing commands.

## Decisions

### 1. Keep `MaterializationMode` as the single source of truth across the pipeline

Config validation will parse `copy` and `hardlink` into the existing `MaterializationMode` enum, and resolved intents, state records, plan printing, and inspection views will carry that enum forward without collapsing back to symlink-only behavior.

Why this choice:
- It keeps the planner and apply engine mode-aware without inventing parallel flags.
- It minimizes accidental mismatches between config, runtime behavior, and persisted state.

Alternatives considered:
- Parsing new modes late inside apply. Rejected because plan/inspection/state would still misrepresent the intended operation.

### 2. Materialize directories recursively for `copy` and `hardlink`

When a rule matches a directory, `copy` will create an independent directory tree with copied files, and `hardlink` will create a mirrored directory tree whose files are hardlinked to the source files. Directory nodes themselves remain ordinary directories; only leaf files are hardlinked.

Why this choice:
- Existing profile rules already match directories such as `Skills/*`, so file-only support would be incomplete immediately.
- It preserves the current mental model that one source asset maps to one target asset path, regardless of mode.

Alternatives considered:
- Restrict `copy` and `hardlink` to files only. Rejected because it would make the new modes unusable for the repo's existing directory-oriented rules.

### 3. Replace symlink-only path verification with mode-aware filesystem snapshots

The state/journal layer will capture structured snapshots for files, directories, and symlinks. File snapshots will include enough identity to detect post-apply drift before undo, and directory snapshots will recursively record entries so verification can prove the target still matches the applied shape.

Why this choice:
- `PathState::File` is too weak for copy and hardlink because it cannot distinguish the desired file from a later manual edit.
- Doctor and undo need the same source of truth to compare current state against what apply created.

Alternatives considered:
- Trust file existence plus managed records. Rejected because undo could then revert modified concrete files silently.

### 4. Reuse one mode-aware reconcile path for plan/apply/doctor/undo

Plan classification will compare the current target against the desired source using mode-aware matching. Apply will materialize via one helper per mode, then run a shared post-apply verification step. Doctor and undo preflight will reuse the same snapshot/match helpers instead of open-coding per-command checks.

Why this choice:
- It keeps safety behavior consistent across commands.
- It avoids one command considering a target healthy while another treats the same target as drifted.

Alternatives considered:
- Keep separate bespoke checks for apply, doctor, and undo. Rejected because multi-mode behavior would drift quickly.

## Risks / Trade-offs

- [Recursive snapshots increase implementation complexity] -> Mitigation: keep the snapshot format narrow and reuse it for verification, doctor, and undo instead of adding separate representations.
- [Hardlink behavior depends on filesystem constraints] -> Mitigation: surface the underlying apply error when a source/target combination cannot be hardlinked and cover the supported same-filesystem path in tests.
- [Recursive copy/hardlink can be slower than symlinks] -> Mitigation: preserve the mode as an explicit operator choice rather than changing the default.

## Migration Plan

1. Update OpenSpec requirements so config, reconcile, and state semantics explicitly allow multi-mode materialization.
2. Extend the Rust data model and filesystem helpers to materialize and verify `copy` and `hardlink`.
3. Add CLI/integration coverage for validation, apply, doctor, and undo with the new modes.
4. Refresh README wording so operator expectations match the shipped behavior.

## Open Questions

- None for this change.
