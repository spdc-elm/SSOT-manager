## Why

Directory materialization in `copy` and `hardlink` mode currently treats every extra filesystem entry as managed drift. That is correct for strict mirroring, but it produces low-value noise on real hosts where platform junk such as `.DS_Store` can appear inside otherwise healthy managed trees.

The system needs an explicit way to declare ignorable paths without baking host-specific exceptions into the core reconciliation model. That keeps drift detection precise by default while letting operators opt into pragmatic ignore policy where their environment requires it.

## What Changes

- Add an optional rule-level `ignore` field that accepts glob patterns relative to the matched asset tree.
- Validate ignore globs during config loading and preserve them in resolved sync intents.
- Apply ignore patterns consistently during reconciliation, verification, and doctor comparisons for directory materializations so ignored extra entries do not cause drift.
- Keep the default behavior strict: the manager will not ship built-in ignore defaults for macOS, Windows, or other host-specific junk files.
- Update config/skill documentation to recommend platform-specific ignore patterns for `copy` and `hardlink` directory rules when operators sync into environments that generate extra metadata files.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `profile-config-resolution`: rules may declare validated ignore globs and resolved intents must preserve them.
- `sync-reconciliation`: directory comparisons and apply verification may ignore configured relative-path matches instead of treating them as drift.
- `managed-state-and-doctor`: doctor drift checks must honor the configured ignore globs for managed directory targets.

## Impact

- Affected code: config parsing/validation, resolved sync intent data model, directory snapshot or comparison logic, reconcile verification, and doctor reporting.
- Affected docs: config schema references, README/example guidance, and the `ssot-manager-config` skill guidance for `copy`/`hardlink` usage on hosts that emit platform metadata files.
- No breaking default behavior: configs without `ignore` remain strict and unchanged.
