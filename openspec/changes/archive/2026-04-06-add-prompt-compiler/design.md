## Context

The current SSOT manager is intentionally mechanical. It reads one YAML config, resolves profiles into filesystem sync intents, and applies reconcile actions through a thin CLI and TUI shell. That shape is good for trust, but it assumes every managed asset already exists as a concrete source file.

Prompt-oriented assets break that assumption. A practical target such as `AGENTS.md` often needs to be compiled from multiple source documents such as `Agents/assistant.md` and `USER.md`, then wrapped in host-specific structure before sync. If that logic lives inside profile rules or external shell scripts, the system loses explainability and starts mixing content generation with filesystem reconciliation.

This change is cross-cutting because it adds a new compiler layer, extends config validation, changes profile preflight semantics for generated assets, introduces new CLI surfaces, and expands the TUI's role from profile-only inspection to profile-centered compile-plus-sync orchestration.

## Goals / Non-Goals

**Goals:**
- Add a prompt compiler layer that is architecturally separate from sync/reconcile logic.
- Keep one binary entry point while separating compiler and sync responsibilities in the Rust library and CLI surface.
- Support deterministic prompt composition from ordered source files, configurable per-input wrappers, configurable output wrappers, and declared variable interpolation.
- Make generated prompt assets easy to target from existing sync profiles through explicit profile prerequisites rather than implicit missing-file behavior.
- Extend the TUI with basic profile-centered prerequisite inspection and compile actions through shared in-process APIs.

**Non-Goals:**
- Executing arbitrary user-provided scripts, shell commands, or language runtimes in the first version.
- Turning the SSOT manager into a general template engine or workflow runner.
- Replacing reconcile/apply safety semantics with compile-time shortcuts.
- Solving every future renderer need now; the first version only needs the common prompt assembly case.

## Decisions

### 1. Keep a single `ssot` binary, but split compiler and sync into separate library modules and CLI subcommands

The shipped executable should remain one binary, but it should expose a distinct `prompt` command tree alongside the existing sync-oriented commands. The Rust library should gain compiler-focused modules rather than embedding composition logic into `reconcile` or `tui`.

Why this choice:
- It preserves one operational entry point while keeping code responsibilities explicit.
- It matches the current thin-shell architecture, where CLI and TUI call shared library code directly.
- It avoids subprocess orchestration inside the TUI.

Alternatives considered:
- A second standalone compiler binary. Rejected for the first iteration because it complicates packaging and TUI integration before the library boundaries are proven.
- Embedding compile behavior into `profile apply`. Rejected because it blurs the line between deterministic filesystem reconciliation and content generation.

### 2. Extend the shared YAML config with optional named prompt compositions and explicit profile prerequisites

The existing config should grow an optional `compositions` map. Each composition declares:
- ordered source inputs
- an output path
- a built-in renderer kind plus wrapper configuration
- optional declared variables

Profiles should also be allowed to declare an optional ordered `requires` list of named compositions. That list becomes the explicit contract that a profile depends on generated prompt outputs before reconcile/apply can proceed.

The config loader remains the single validation entry point. Composition output paths must resolve under the shared `source_root`, so generated assets can be selected by existing profiles without introducing multi-root profile semantics or virtual sources.

Why this choice:
- It keeps one source of truth for both generated prompt assets and sync profiles.
- It makes generated-asset dependencies explicit instead of forcing reconcile to infer intent from missing files under `build/`.
- It avoids immediately reopening profile composition or multi-root rule design.

Alternatives considered:
- Separate compiler config file. Rejected because it would fragment the source of truth and complicate TUI/CLI coordination.
- Writing compiled outputs outside `source_root`. Rejected for V1 because the current profile model has one effective `source_root`, so external outputs would force more config redesign than the compiler itself.

### 3. Treat prompt prerequisites as explicit profile preflight, not an implicit side effect of `apply`

Profile planning and apply should validate required compositions before filesystem reconcile begins. A missing or stale required composition must surface as a blocking prerequisite issue rather than degrading into a generic `asset_not_found` warning from ordinary source discovery.

The default behavior should remain phase-separated:
- `prompt build <name>` compiles one named composition
- `profile plan/apply <name>` checks required compositions and blocks if they are missing or stale

This change does not require a compile-on-apply shortcut in the CLI. If a convenience orchestration path is added later, it should still expose compile and sync as distinct phases in status output.

Why this choice:
- It keeps sync trustworthy by making generated dependencies explicit.
- It prevents silent or weakly signaled failure when a generated source file has not been compiled yet.
- It preserves a clean mental model: compile first, then sync.

Alternatives considered:
- Auto-compiling inside every `profile apply`. Rejected because it hides two distinct failure modes behind one action.
- Treating missing generated outputs as ordinary asset discovery warnings. Rejected because that under-specifies generated prerequisites and weakens safety.
### 4. Model the first renderer set as built-in deterministic composition, not arbitrary scripting

The first compiler implementation should support built-in rendering only:
- ordered concatenation
- optional per-input wrappers
- optional outer wrapper
- declared variable interpolation in wrapper templates and static text fragments

Compilation should be a pure function of declared inputs and declared variables. Wrapper templates should fail fast when they reference undefined variables.

Why this choice:
- It covers the dominant prompt assembly case discussed for this project.
- It keeps preview, testing, and explanation straightforward.
- It avoids dragging an embedded language runtime into the first milestone.

Alternatives considered:
- User-provided shell or Python hooks. Rejected because they would immediately weaken determinism, portability, and explainability.
- Starting with a heavier embedded runtime such as JavaScript. Rejected because the common case does not need that complexity.

### 5. Reserve a renderer abstraction that can grow into scripted rendering later, but reject script renderers in V1

The compiler should be structured around a renderer abstraction so future work can add new built-in renderers or, later, a constrained script-backed renderer. The public config and CLI behavior in this change must still reject `script` or any other non-built-in renderer kind.

Why this choice:
- It keeps the design extensible without pretending that arbitrary script execution is solved.
- It prevents a later script experiment from forcing a second compiler rewrite.

Alternatives considered:
- Hard-coding one ad hoc concat implementation. Rejected because it would make later renderer growth messy.
- Exposing script renderers immediately. Rejected because the boundary and safety model are not ready.

### 6. Keep the first TUI prompt support profile-centered and prerequisite-oriented
The TUI should continue to be organized around selected profiles. For the first iteration, it does not need a separate full composition browser. Instead, the selected profile view should surface:
- required compositions and their current status
- the generated output path for each prerequisite
- a basic action to compile the selected profile's required compositions

The TUI must continue to call library functions directly instead of parsing CLI output or hiding compile failure inside apply. A compile-dependencies action should materialize generated assets first; only after a successful compile should the operator proceed to profile plan/apply against those outputs.

Why this choice:
- It preserves the TUI as a shell over deterministic engines rather than a second control plane.
- It makes compile failures legible before sync is attempted.
- It gives the user a comfortable path without requiring a second navigation model before the basics are proven.

Alternatives considered:
- Making the TUI shell out to `ssot prompt ...` and `ssot profile ...`. Rejected because it duplicates wiring and creates brittle text-coupled behavior.
- Auto-compiling during every apply without showing compile state. Rejected because it hides too much coupling.
- Building a separate composition-first TUI pane from day one. Rejected because the first user need is profile success, not a full secondary browser.

## Risks / Trade-offs

- [Generated outputs live under `source_root`, which can clutter the repo workspace] -> Mitigation: constrain outputs to a dedicated generated path, document that it should be gitignored, and keep generated assets clearly non-editable.
- [Built-in renderers may feel too narrow for edge cases] -> Mitigation: reserve the renderer abstraction now, but keep the first shipped surface intentionally small and explainable.
- [Shared config becomes broader and easier to misuse] -> Mitigation: keep composition schema narrow, validate output paths strictly, and reject unsupported renderer kinds early.
- [Generated prerequisite freshness can be underspecified] -> Mitigation: require an explicit compiler status check in library APIs and block reconcile/apply when prerequisites are missing or stale.
- [TUI scope can expand too quickly] -> Mitigation: keep the first prompt UI profile-centered and prerequisite-oriented, not a general composition workbench.

## Migration Plan

1. Extend OpenSpec requirements for composition recipes, compiler CLI behavior, profile prerequisite validation, and profile-centered TUI orchestration.
2. Add compiler-oriented config types, validation, inspection data, and rendering modules in the Rust library.
3. Add `prompt` CLI subcommands for inspection, preview, and build.
4. Update profile planning/apply preflight so required compositions block sync when they are missing or stale.
5. Update the TUI to expose profile prerequisite status and compile-dependencies actions through shared library calls.
6. Refresh example config, README guidance, and test coverage to show compile-then-sync workflows.

## Open Questions

- What exact placeholder syntax should the built-in renderer adopt for declared variables?
- Should `prompt build` support building all recipes in one invocation, or only named recipes in the first version?
