## Why

The current SSOT manager cleanly handles deterministic sync, but it assumes every managed asset already exists as a file in the source tree. Prompt-style assets break that assumption because many real targets should be compiled from multiple source documents, host-specific wrappers, and a small amount of injected metadata before they are synced.

We need a prompt compiler layer now so prompt assembly does not get smuggled into sync rules or ad hoc shell scripts. The system should stay deterministic and explainable while covering the common case of ordered concatenation, XML-style wrappers, and controlled variable injection.

## What Changes

- Add a prompt composition model that defines named recipes for compiling generated prompt assets from ordered source files and declared variables.
- Let sync profiles declare explicit dependencies on named prompt compositions instead of treating generated assets as ordinary missing source files.
- Add a compiler CLI surface under the same binary so operators can inspect and build generated prompt outputs without invoking sync actions.
- Support a conservative first renderer set: ordered concatenation, configurable outer wrappers, configurable per-input wrappers, and variable interpolation from declared values.
- Keep prompt compilation logic separate from reconcile/apply logic, and require profile preflight to surface missing or stale prompt prerequisites before sync proceeds.
- Extend the TUI with profile-centered prompt prerequisite status and a basic compile-dependencies action through shared library APIs rather than shelling out.
- Reserve a script renderer extension point in the design, but do not allow arbitrary user-defined script execution in the first version.

## Capabilities

### New Capabilities
- `prompt-composition`: Define deterministic prompt composition recipes that read declared source files in order, apply built-in wrappers, inject declared variables, and emit generated prompt assets.
- `prompt-compiler-cli`: Expose read/build compiler commands for listing recipes, showing recipe structure, previewing compiled output, and materializing generated prompt assets without mutating sync targets.

### Modified Capabilities
- `profile-config-resolution`: Extend config validation so the shared YAML model can declare prompt composition recipes, generated asset locations, and explicit profile dependencies on named compositions.
- `sync-reconciliation`: Extend planning and apply preflight so profiles with generated prompt prerequisites surface missing or stale compositions as blocking dependency issues instead of generic missing-asset warnings.
- `terminal-ui-shell`: Extend the TUI so operators can see prompt prerequisite status for the selected profile and run a basic compile-dependencies action before sync/apply using shared in-process APIs.

## Impact

- Adds a prompt compiler layer to the Rust workspace with a clear separation between composition/build responsibilities and sync/materialization responsibilities.
- Expands the config schema and validation path to include prompt composition declarations, generated output locations, and profile-to-composition dependency declarations.
- Adds compiler-oriented CLI flows and profile prerequisite handling while preserving the existing safety model for reconcile/apply.
- Introduces new generated-output handling, dependency preflight behavior, and test coverage for recipe resolution, rendering, and profile-centered compile-plus-sync orchestration.
