## 1. Config And Data Model

- [x] 1.1 Extend the shared YAML config model and validation path to accept optional `compositions`, built-in renderer settings, declared variables, output paths constrained under `source_root`, and profile `requires` references to named compositions
- [x] 1.2 Add compiler-focused Rust types for composition recipes, renderer configuration, compile inputs, compile results, and composition readiness status without coupling them to reconcile structs
- [x] 1.3 Update config/example fixtures so at least one prompt composition compiles `Agents/assistant.md` and `USER.md` into a generated prompt asset under the source tree

## 2. Prompt Compiler Core And CLI

- [x] 2.1 Implement compiler inspection helpers for listing compositions and showing the effective definition of a named composition
- [x] 2.2 Implement the built-in deterministic renderer for ordered concatenation, per-input wrappers, outer wrappers, and declared variable interpolation with undefined-variable errors
- [x] 2.3 Implement composition readiness checks that can report ready, missing, or stale outputs for profile and TUI prerequisite handling
- [x] 2.4 Add `prompt` CLI subcommands for list, show, preview, and build using shared compiler library APIs rather than ad hoc file writes
- [x] 2.5 Ensure generated prompt outputs are materialized under their declared paths with parent-directory creation and clear success/error reporting

## 3. Profile Preflight And TUI Integration

- [x] 3.1 Update profile plan/apply preflight so required compositions are checked before normal reconcile and missing or stale prerequisites block sync
- [x] 3.2 Extend the TUI's selected-profile detail so it shows required prompt compositions, their status, and generated output paths
- [x] 3.3 Add a TUI compile-dependencies action that runs prompt compilation for the selected profile's required compositions through library APIs and surfaces compile failures without reporting false sync success

## 4. Verification And Documentation

- [x] 4.1 Add unit and integration coverage for composition config validation, deterministic render ordering, readiness status, variable interpolation failures, and CLI preview/build behavior
- [x] 4.2 Add integration coverage for profile prerequisite blocking when required compositions are missing or stale
- [x] 4.3 Add TUI-focused tests for profile prerequisite inspection and compile-dependencies action state/error handling
- [x] 4.4 Update README and workflow documentation to explain profile `requires`, the compile-versus-sync split, the built-in renderer scope, and the first-version rejection of script renderers
