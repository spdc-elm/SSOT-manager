## Why

The reconciler currently reasons about each concrete target path in isolation, which means it can miss a dangerous topology where the target's parent directory is itself a symlink back into the source tree. In that case a seemingly ordinary sync target can resolve back onto the source asset being managed, creating self-referential mutation paths that are unsafe even when the leaf path is not recorded as managed.

## What Changes

- Add an explicit reconcile safety rule that blocks targets whose effective materialization path overlaps the source asset path, including cases introduced by symlinked ancestor directories.
- Classify these self-referential targets as non-forceable `danger` actions so both CLI and TUI apply flows refuse them before mutation.
- Add regression tests that build the rare "parent directory is a symlink back into source" topology and assert that plan/apply block it.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `sync-reconciliation`: tighten planner safety so self-referential source/target overlaps are detected before apply.

## Impact

- Affected code: reconcile planning, path resolution helpers, and flow regression tests
- Affected specs: `openspec/specs/sync-reconciliation/spec.md`
- Behavioral impact: some previously under-specified symlink topologies will now be reported as explicit blocking dangers instead of reaching lower-level mutation paths
