## Context

The current TUI shell always renders a selected profile on the left and its detail on the right, but it still behaves like a single-focus screen. Profile movement and detail reading compete for the same limited key vocabulary, and the recent stopgap of binding detail scrolling to `PageUp`/`PageDown` makes long inspection output technically reachable without making it legible.

That stopgap already exists in the working tree: long right-pane content can now be scrolled with dedicated page-navigation keys, and long editor collection popups keep the selected entry visible. However, the shell still lacks:
- an explicit main-pane focus model
- a visible overflow affordance in the right pane
- a clean internal boundary between shell state, editor state, input handling, rendering, and tests

At the same time, the TUI implementation has grown into a single large source file. The pane-focus change is the first point where continuing to extend that file would materially worsen maintainability.

This matters most on long `plan` output, where operators want two different workflows:
- quickly skim several profiles from the left pane
- temporarily stop browsing and read one profile's detail deeply

The shell already has a separate edit mode with stronger interaction boundaries. The main inspection shell now needs a lighter version of that explicitness so navigation remains understandable as the TUI grows.

## Goals / Non-Goals

**Goals:**
- Add an explicit main-shell focus model that distinguishes left-pane browsing from right-pane detail inspection.
- Preserve fast preview behavior so changing the selected profile still updates the right pane immediately in browse mode.
- Let operators enter a focused detail-reading state where familiar vertical keys scroll long content instead of changing profile selection.
- Make long inspection output visibly scrollable through a right-pane overflow cue or position indicator.
- Keep reconcile and compile actions bound to the selected profile without introducing a second orchestration path.
- Use this change to perform a minimal TUI module split so the new shell state machine does not further bloat the existing monolithic file.

**Non-Goals:**
- Replacing the current profile-centered layout with a route-based TUI.
- Clearing the detail pane whenever focus leaves it; preview should remain visible in browse mode.
- Redesigning the profile editor interaction model beyond any shared rendering affordances reused incidentally.
- Adding mouse handling, split-pane resizing, or generalized scrollbars across every popup in this change.
- Introducing a generalized MVC/presenter framework or broad abstraction layer for the TUI.

## Decisions

### 1. Introduce an explicit shell focus enum for the non-editing TUI

The main inspection shell will track whether navigation focus is on:
- the profile list (`BrowseProfiles`)
- the detail pane (`InspectDetail`)

Edit mode remains a higher-priority state layered above both of these shell states.

Why this choice:
- The current shell has grown beyond what a single implicit focus can express cleanly.
- It gives `Enter`, `Esc`, and `j/k` stable, comprehensible meanings without making browse mode heavier.
- It avoids turning ad hoc scroll flags into a hidden second state machine.

Alternatives considered:
- Keep a single shell mode and add more dedicated scroll keys only. Rejected because it preserves the discoverability problem and leaves `j/k` overloaded conceptually.
- Require `Enter` before any detail is shown. Rejected because it would slow down quick preview browsing for no real benefit.

### 2. Keep live right-pane preview in browse mode, and let `Enter` promote that preview into focused inspection

Browse mode will continue to render the selected profile's `show`, `plan`, or `doctor` view on the right. Pressing `Enter` from the main shell will not open a new route; it will move focus into the existing detail pane for the currently selected profile and current tab.

Pressing `Esc` or `h` from detail focus will return to browse mode while preserving:
- the selected profile
- the active detail tab
- the current visible right-pane content

Why this choice:
- It keeps the shell fast for scanning and deliberate for deep reading.
- It matches the existing mental model better than route-style open/close behavior.
- It makes focus a navigation concern rather than a data-loading concern.

Alternatives considered:
- Clear the right pane on `Esc`. Rejected because it throws away useful context and makes browse mode visually emptier without simplifying implementation meaningfully.
- Open detail focus in a modal or fullscreen overlay. Rejected because the screen already has a natural right-hand inspection surface.

### 3. Rebind vertical navigation keys by focus, not by view

In browse mode:
- `j`/`k` and `Up`/`Down` move between profiles

In detail focus:
- `j`/`k` and `Up`/`Down` scroll vertically within the right pane
- `PageUp`/`PageDown`/`Home`/`End` remain available as larger movement keys

Tab and horizontal navigation for changing `Show` / `Plan` / `Doctor` remain available from the shell and will keep working in both browse mode and detail focus. View changes reset right-pane scroll to the top.

Why this choice:
- It gives the most common keys the most common meaning inside the active pane.
- It aligns with how terminal users expect focused panes to behave.
- It avoids forcing operators onto only page-sized scroll keys for ordinary reading.

Alternatives considered:
- Disable tab switching while detail-focused. Rejected because the focused profile may still need cross-view inspection.
- Reserve `j/k` for profiles forever and use only page keys in detail focus. Rejected because it keeps right-pane reading clumsy.

### 4. Represent detail overflow explicitly with a lightweight indicator instead of a full widget rewrite

The detail pane will render a visible overflow cue when content exceeds the viewport. This can be a narrow gutter-style scrollbar, a top/bottom continuation marker, a line-position readout, or a combination that is cheap to render in `ratatui` while remaining obvious on small terminals.

The indicator should be driven by the existing scroll offset plus current visible height, not by a separate measurement pass.

Why this choice:
- The usability problem is primarily one of legibility and affordance, not raw scroll capability.
- A lightweight indicator fits the current hand-rolled TUI better than introducing a larger list/viewport abstraction prematurely.
- It leaves room to reuse the same rendering idea in editor overlays later if needed.

Alternatives considered:
- Postpone all indicators and rely on footer help text. Rejected because discoverability is the problem.
- Rebuild the detail pane around a different widget framework immediately. Rejected because the interaction model can be clarified without a larger rendering rewrite.

### 5. Perform a minimal responsibility-based TUI split before deepening the shell state machine

Before or while implementing pane focus, the single `src/tui.rs` file should be split into a small `src/tui/` module tree with boundaries close to current responsibilities, such as:
- `mod.rs` for public entry points and light wiring
- `state.rs` for TUI state structs and state-local helpers
- `input.rs` for key handling and state transitions
- `render.rs` for screen and overlay rendering
- `editor.rs` for editor-specific state transitions or helpers if that keeps the shell path simpler
- `tests.rs` (or a small set of module-local test files) for current unit-style TUI tests

This split is intentionally conservative: it keeps the current TUI design, private visibility model, and test style, but stops the shell/input/render/editor concerns from continuing to accrete in one file.

Why this choice:
- The current file is already large enough that adding pane focus directly into it would make future maintenance harder.
- The upcoming change naturally cuts across state, input, rendering, and tests, which makes it a good seam for a responsibility-based split.
- Keeping tests near the TUI module preserves access to private helpers without turning many internal functions public just to satisfy integration tests.

Alternatives considered:
- Leave `src/tui.rs` intact and only refactor after pane focus lands. Rejected because this change is exactly where the file will become meaningfully harder to reason about.
- Move all TUI tests to top-level `tests/`. Rejected because many current tests intentionally exercise private render and state-transition helpers.
- Design a larger reusable TUI framework first. Rejected because the project does not need that abstraction level yet.

## Risks / Trade-offs

- [Two shell focus states plus edit mode can become confusing internally] -> Mitigation: keep shell focus as a tiny explicit enum and make edit mode continue to short-circuit main-shell key handling.
- [Visible scroll indicators may look noisy in small terminals] -> Mitigation: prefer a minimal indicator tied only to actual overflow and suppress it when content fits.
- [Action keys firing while detail is focused could feel inconsistent if focus looks modal] -> Mitigation: document that focus changes navigation semantics, not the selected profile target for shell actions.
- [Per-view scroll state can become stale on profile/view transitions] -> Mitigation: reset or clamp scroll on profile change, view change, refresh, and any state transition that swaps the rendered content.
- [A module split inside the same behavior change can complicate review] -> Mitigation: keep the split responsibility-based and shallow, and avoid mixing it with new abstractions unrelated to pane focus.

## Migration Plan

1. Add a `terminal-ui-shell` spec delta for pane focus semantics and detail overflow affordances.
2. Split the current TUI implementation into a small responsibility-based module tree without changing external behavior.
3. Extend the TUI state model with explicit shell focus and deterministic focus transitions.
4. Update main-shell key handling so vertical movement is dispatched by focus state while edit mode keeps priority.
5. Add right-pane overflow rendering and clamp/reset logic for scroll offsets.
6. Add focused tests for browse/detail transitions, long-detail scrolling, and visible overflow cues.
7. Update README navigation guidance to describe browse mode versus detail focus mode.

## Open Questions

- None for the proposal. The main product choice is already made: preview remains live in browse mode, and `Enter`/`Esc` only control focus.
