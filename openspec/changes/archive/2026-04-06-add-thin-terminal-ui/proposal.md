## Why

The CLI MVP now proves the sync loop, but it still hides the product behind memorized commands and text dumps. A thin TUI is the fastest way to make the manager feel inspectable and tangible without inventing a second source of truth.

## What Changes

- Add a terminal UI that presents profiles, resolved profile details, plan output, and doctor results using the same inspection and reconcile logic as the CLI.
- Let operators trigger existing safe actions from the TUI, including `plan`, `apply`, `doctor`, and `undo`, without duplicating reconcile behavior.
- Keep the TUI intentionally thin: browse state, preview actions, and invoke existing commands, but do not implement interactive config editing or separate orchestration logic.
- Depend on the CLI inspection change to provide stable profile metadata and explanation models before the TUI is built.

## Capabilities

### New Capabilities
- `terminal-ui-shell`: Present profile inspection and reconcile output in a navigable terminal interface while delegating behavior to the existing deterministic engine.

### Modified Capabilities
- None.

## Impact

- Adds a TUI entry point and screen flow under `SSOT-manager/`.
- Introduces a terminal UI dependency and view-state layer over existing library data.
- Extends spec coverage for TUI navigation and action handling.
- Makes the product visibly usable without broadening the configuration or sync model.
