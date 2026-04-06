## Context

The draft treats a TUI as a convenience layer over the deterministic core, not a second control plane. After the CLI inspection layer exists, the manager will have enough structured data to render profiles, resolved state, and reconcile previews in a terminal interface without inventing new business logic.

This change should make the tool feel tangible quickly, but it should stay thin: browse, preview, and trigger existing safe actions. The existing engine and state model remain authoritative.

## Goals / Non-Goals

**Goals:**
- Add a terminal UI entry point that lets operators browse profiles and inspect their current state.
- Reuse shared inspection and reconcile library functions instead of shelling out to CLI text output.
- Support previewing plan and doctor results and invoking existing safe actions such as `apply` and `undo`.
- Keep the UI understandable and low-risk enough to validate quickly on this repo.

**Non-Goals:**
- Interactive config editing, toggling, or writing YAML from the TUI.
- A separate state model, background daemon, or event loop that changes reconcile semantics.
- Full-screen workflow customization, theming, or plugin-style TUI extensibility.
- Asset-level browsing beyond what is needed to understand a selected profile.

## Decisions

### 1. Add a single thin TUI command on top of the Rust library

Expose a new CLI entry point such as `ssot tui` that starts a terminal UI from the same binary. The UI should call shared inspection and reconcile functions directly through the library crate.

Why this choice:
- It preserves a single executable and shared code path.
- It avoids fragile subprocess orchestration or parsing CLI text.
- It keeps future testing focused on the library behavior and a small view layer.

Alternatives considered:
- Build the TUI as a separate crate or binary. Rejected because it would duplicate startup, config loading, and state wiring too early.

### 2. Keep the first TUI screen model simple and profile-centered

The initial layout should focus on one selected profile at a time:
- profile list pane
- detail pane with tabs or modes for show, plan, and doctor
- footer or action bar for refresh, apply, undo, and quit

Why this choice:
- It aligns directly with the draft's TUI role.
- It makes the UI useful immediately without solving general asset exploration.
- It reduces layout complexity for the first iteration.

Alternatives considered:
- Multi-panel asset explorer first. Rejected because there is no asset-level inspection API yet and it adds scope without proving the main user value.

### 3. Reuse existing safety semantics instead of adding UI-only confirmations

The TUI should invoke the same apply and undo functions the CLI uses and render their existing safe or blocked outcomes. It may present a confirmation step for destructive-looking actions, but it must not bypass `danger` blocking or add an alternate mutation path.

Why this choice:
- Safety guarantees must remain engine-level, not UI-level.
- It keeps the TUI honest as a shell over the deterministic core.

Alternatives considered:
- Let the TUI offer force actions or different danger handling. Rejected because that would create a second source of truth for safety behavior.

### 4. Choose a standard Rust TUI stack

Use a focused terminal UI stack such as `ratatui` with `crossterm` for rendering and input handling.

Why this choice:
- It is the common, maintained path for Rust terminal interfaces.
- It supports a thin full-screen UI without committing to a heavy framework.
- It keeps input handling and redraw logic straightforward for a profile-centered shell.

Alternatives considered:
- A line-oriented REPL or menu prompt. Rejected because it would not materially improve inspectability over the existing CLI.

## Risks / Trade-offs

- [The TUI can become a second orchestration surface] -> Mitigation: route every action through existing library functions and keep the UI read-mostly.
- [Terminal UI dependencies add maintenance overhead] -> Mitigation: choose a common crate stack and keep the screen model intentionally small.
- [The first TUI may expose too much text if plan or doctor output is verbose] -> Mitigation: build around shared structured data and summaries instead of embedding raw command text blobs.

## Migration Plan

1. Finish and land the CLI inspection change so the TUI has stable shared data models.
2. Add the TUI dependency stack and a `ssot tui` entry point.
3. Build a profile list and detail view for show, plan, and doctor data.
4. Wire existing `apply` and `undo` behavior into the TUI action flow.
5. Dogfood the TUI against this repo's sample config and refine layout based on real plan and doctor output.

## Open Questions

- Should the first TUI default to the sample config path when run inside `SSOT-manager/`, or always require explicit config input like the CLI?
- How much of the existing plan detail belongs in the main detail pane versus an expandable full-detail view?
