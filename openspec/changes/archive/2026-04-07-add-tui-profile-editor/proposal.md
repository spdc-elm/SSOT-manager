## Why

The current TUI is useful for inspecting profiles and running safe reconcile actions, but any profile change still requires editing YAML by hand. For this project, that creates an unnecessary gap between "see what the profile does" and "adjust the profile definition", especially when the desired change is simple and mechanical.

The user is willing to accept normalized config rewrites instead of comment-preserving round trips. That makes it practical to add a constrained profile editor now without first building a lossless YAML document model.

## What Changes

- Add a profile editing flow to the TUI so an operator can create, edit, and remove profile-local fields through structured terminal interactions instead of hand-editing YAML.
- Support normalized config rewrites when saving from the TUI, with validation and explicit failure handling before any rewritten config becomes active.
- Keep editing scoped to profile configuration data rather than turning the TUI into a general-purpose YAML editor.
- Preserve the existing inspection, plan, doctor, compile, apply, and undo flows while integrating an edit mode and unsaved-change handling.

## Capabilities

### New Capabilities
- `profile-config-editing`: Structured creation and editing of profile definitions in the TUI, including draft state, validation, normalized YAML rewrite, and save/discard behavior.

### Modified Capabilities
- `terminal-ui-shell`: The terminal UI will gain an edit entry point and editing interactions in addition to the current inspection and reconcile actions.

## Impact

- Affected code: `SSOT-manager/src/tui.rs`, `SSOT-manager/src/config.rs`, CLI/TUI support types, and related tests.
- Affected specs: new `profile-config-editing` capability plus a delta for `terminal-ui-shell`.
- Behavioral impact: saved config files may be reformatted and comments will not be preserved after TUI edits.
