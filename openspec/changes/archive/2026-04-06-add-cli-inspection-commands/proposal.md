## Why

The current MVP can mutate and verify state, but it still makes users inspect the system through a narrow set of commands. Before adding a TUI, the manager needs a richer read-only inspection surface that can explain profiles, resolved rules, and planned outcomes in a stable way.

## What Changes

- Add read-only inspection commands for profile discovery and explanation, centered on `profile list`, `profile show <name>`, and `profile explain <name>`.
- Introduce a shared inspection layer in the Rust library so CLI output and future TUI views consume the same resolved profile and plan details.
- Support machine-readable output for the new inspection commands so later UI work does not depend on parsing human-formatted terminal text.
- Keep apply semantics unchanged; this change expands visibility, not mutation scope.

## Capabilities

### New Capabilities
- `profile-inspection-cli`: List profiles, show effective profile configuration, and explain resolved profile state through human-readable and machine-readable inspection commands.

### Modified Capabilities
- None.

## Impact

- Adds new CLI subcommands and output paths in `SSOT-manager/src/cli.rs`.
- Adds a reusable inspection data model and rendering path in the Rust library.
- Extends spec coverage for read-only profile inspection behavior.
- Provides the stable information contract the planned thin TUI will consume next.
