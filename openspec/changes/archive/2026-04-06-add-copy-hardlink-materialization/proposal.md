## Why

The SSOT manager currently rejects `mode: copy` and `mode: hardlink`, which forces every managed target to remain a symlink even when an operator needs concrete file materialization. That restriction now blocks real apply workflows because the planner, verifier, doctor, and undo path already assume a single mode and cannot represent or recover mixed managed states safely.

## What Changes

- Accept `mode: copy` and `mode: hardlink` in profile rules alongside `symlink`.
- Plan, apply, verify, and inspect managed targets according to each rule's declared materialization mode instead of assuming symlink-only behavior.
- Extend managed state, doctor, and undo behavior so recorded targets can be validated and restored for symlink, copy, and hardlink materializations.
- Update CLI-facing docs and regression coverage to reflect multi-mode apply behavior.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `profile-config-resolution`: allow `copy` and `hardlink` as valid rule modes and preserve them in resolved intents.
- `sync-reconciliation`: classify and apply desired targets against the live filesystem using the declared materialization mode, with post-apply verification for symlink, copy, and hardlink outcomes.
- `managed-state-and-doctor`: record, diagnose, and undo managed targets for copy and hardlink materializations in addition to symlinks.

## Impact

- Affected code: `SSOT-manager/src/config.rs`, `SSOT-manager/src/reconcile.rs`, `SSOT-manager/src/state.rs`, `SSOT-manager/src/inspection.rs`, `SSOT-manager/src/tui.rs`, `SSOT-manager/README.md`
- Affected tests and fixtures under `SSOT-manager/tests/`
- No new external dependencies expected
