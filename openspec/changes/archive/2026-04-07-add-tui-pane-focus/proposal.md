## Why

The current TUI still treats the right-hand inspection pane as a passive preview. That works for short content, but it breaks down on large plans and long profile details because operators cannot clearly tell when the detail pane is scrollable, which pane currently owns navigation keys, or how to move between browsing profiles and inspecting one profile deeply.

Now that the profile editor exists, this ambiguity is more costly: the shell needs a clearer interaction model before more TUI surface area accumulates around the current "always-previewing" behavior.

## What Changes

- Add an explicit pane-focus interaction model to the main TUI shell so operators can switch between profile-list navigation and focused detail inspection without overloading the same keys ambiguously.
- Let the detail pane enter a focused inspection state from the selected profile and leave that state explicitly, while preserving fast left-pane browsing when not focused.
- Add visible overflow affordances for long inspection content, including a right-pane scroll indicator or equivalent position cue, so long `show`, `plan`, and `doctor` views read as navigable rather than clipped.
- Clarify how shell-level keys such as `j`/`k`, `Enter`, and `Esc` behave in browse mode versus detail-focus mode.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `terminal-ui-shell`: The terminal UI navigation model will gain explicit main-pane focus semantics and visible detail-overflow affordances for long inspection content.

## Impact

- Affected code: `src/tui.rs`, TUI rendering helpers, input/state handling, and TUI-focused tests.
- Affected specs: delta for `terminal-ui-shell`.
- Behavioral impact: shell navigation keys will become mode-dependent in the main inspection UI, with `Enter` and `Esc` participating in pane focus transitions instead of only edit/delete flows.
