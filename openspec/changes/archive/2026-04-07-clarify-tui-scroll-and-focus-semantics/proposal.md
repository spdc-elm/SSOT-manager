## Why

The recently shipped TUI behavior clarified pane focus and added visible scrolling affordances in code, but two parts of that behavior are still underspecified at the OpenSpec level: the precise `h` semantics while detail inspection is focused, and the expectation that long editing popups should visibly advertise overflow instead of only keeping the selected entry in view. If these remain implicit, later refactors can regress them without clearly violating the main product contract.

## What Changes

- Clarify main-shell detail-focus navigation so `h` moves to the previous inspection tab while detail focus remains active, and only leaves detail focus when the current tab is already `Show`.
- Strengthen TUI inspection requirements so long right-pane content is expected to show a visible scrollbar-like overflow affordance, not only a generic cue.
- Extend profile editing requirements so long collection popups visibly advertise overflow in addition to keeping the selected entry visible.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `terminal-ui-shell`: Tighten detail-focus key semantics and visible overflow affordance expectations for the inspection pane.
- `profile-config-editing`: Require visible overflow affordances for long collection editors in addition to selected-entry visibility.

## Impact

- Affected specs: `openspec/specs/terminal-ui-shell/spec.md`, `openspec/specs/profile-config-editing/spec.md`
- Affected code: no new implementation is required immediately; the current TUI implementation already matches the intended behavior and should become the documented baseline
