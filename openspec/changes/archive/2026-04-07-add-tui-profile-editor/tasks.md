## 1. Raw Config Editing Foundation

- [x] 1.1 Refactor the config layer to expose a serializable raw/editable config model for YAML read and write in addition to the validated runtime `Config`
- [x] 1.2 Add a shared validate-from-raw path so candidate TUI edits can reuse existing config validation before save
- [x] 1.3 Add atomic config rewrite helpers that persist normalized YAML without switching the TUI to a broken candidate config on failure

## 2. TUI Profile Draft And Editing Flow

- [x] 2.1 Extend the TUI app state with profile edit mode, draft structs, and selection state for creating, editing, renaming, and deleting profiles
- [x] 2.2 Add structured editing controls for profile-local scalar fields and collection summaries, including `source_root`, `requires`, and ordered rules
- [x] 2.3 Add focused collection editors for nested profile data such as `requires`, rule destinations, and rule tags without introducing a full route-based TUI rewrite
- [x] 2.4 Gate compile, apply, undo, and refresh actions so they only run from the normal inspection shell and not while a draft is active

## 3. Save, Discard, And Delete Safety

- [x] 3.1 Implement draft snapshot comparison and unsaved-change confirmation for leaving profile edit mode
- [x] 3.2 Implement save flow that applies the draft into the raw config, validates it, rewrites YAML atomically, reloads TUI state, and re-anchors selection
- [x] 3.3 Implement confirmed profile deletion and ensure cancel/delete outcomes leave selection and inspection state coherent
- [x] 3.4 Surface validation and write failures inside the TUI while keeping the draft open for correction

## 4. Verification And Documentation

- [x] 4.1 Add config-layer tests for raw config serialization, validate-from-raw behavior, and normalized rewrite expectations
- [x] 4.2 Add TUI-focused tests for entering edit mode, editing nested collections, unsaved-change prompts, save success, save failure, and delete confirmation
- [x] 4.3 Update README and operator guidance to describe the new profile edit flow and the normalized rewrite/no-comment-preservation behavior
