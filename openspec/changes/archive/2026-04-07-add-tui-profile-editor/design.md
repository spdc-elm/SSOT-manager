## Context

The current SSOT manager TUI is intentionally thin. It loads a validated `Config`, shows profile-centered inspection data, and invokes reconcile actions such as compile, apply, undo, and refresh. It does not currently own any mutable config-editing state.

Adding profile editing crosses multiple concerns at once:
- the TUI needs draft state, edit navigation, and unsaved-change handling
- the config layer needs a write path in addition to read-time validation
- saving must not accidentally convert the user's config into a semantically different runtime-only shape

The user accepts normalized YAML rewrites and does not require comment preservation. That removes the need for a lossless YAML AST editor, but it does not remove the need to distinguish between:
- runtime validated config used for planning and apply
- editable config data that matches the YAML schema and can be serialized back to disk safely

## Goals / Non-Goals

**Goals:**
- Add a simple TUI flow for editing profile definitions without hand-editing YAML.
- Keep editing scoped to the `profiles` section and profile-local fields.
- Save by validating first, then atomically rewriting the config file in normalized YAML form.
- Preserve the current profile-centered inspection and reconcile workflow around the new edit mode.
- Make unsaved edits explicit through draft snapshots and save/discard/cancel behavior.

**Non-Goals:**
- Preserving comments, blank-line layout, key ordering beyond deterministic normalized output, or other lossless YAML round-tripping guarantees.
- Turning the TUI into a general-purpose YAML editor.
- Editing top-level `source_root`, `version`, or `compositions` in the first iteration.
- Adding a broad route/worker architecture like `cc-switch-cli`; the feature should fit the existing thin-shell TUI.

## Decisions

### 1. Introduce a serializable raw config model for editing, separate from runtime `Config`

The TUI save path will operate on a serializable raw config structure that mirrors the YAML schema. Validation will continue to produce the runtime `Config`, but the saved file will be written from the raw editable model rather than from the normalized runtime model.

Why this choice:
- The current runtime `Config` resolves paths into `PathBuf`s and stores derived values such as `config_dir`, which are correct for execution but not appropriate as the source of rewritten YAML.
- Writing the runtime `Config` back directly would tend to force absolute-path and runtime-only state into the file.
- A raw editable model lets save reuse the existing validation pipeline while keeping the persisted file aligned with the YAML schema.

Alternatives considered:
- Serialize the validated runtime `Config` back to YAML. Rejected because it would blur execution state and source config shape.
- Build a full YAML AST/document editor. Rejected because normalized rewrites are acceptable and the additional complexity is unnecessary for this change.

### 2. Limit the first editor to `profiles` and profile-local fields

The first TUI editor will support:
- creating a new profile
- renaming or deleting a profile
- editing profile `source_root`
- editing profile `requires`
- editing ordered profile rules, including `select`, `to`, `mode`, `enabled`, `tags`, and `note`

Top-level `version`, top-level `source_root`, and `compositions` will remain outside the editing scope for now.

Why this choice:
- It matches the user's stated need: a simple profile editor that acts as a graphical operation surface for the config file.
- It keeps the UI aligned with the existing profile-centered mental model.
- It avoids dragging prompt-composition editing and top-level config refactoring into the first milestone.

Alternatives considered:
- Edit the full YAML config from the TUI. Rejected because that becomes a general config workbench rather than a focused profile editor.
- Restrict the feature to editing only existing profiles and forbid create/delete. Rejected because create/delete are low-friction extensions once profile-local editing exists and are part of a practical graphical config workflow.

### 3. Keep the TUI architecture lightweight with mode-specific draft state and local overlays

Instead of adopting a large route-based TUI architecture, the current `TuiApp` will gain explicit edit modes and small overlay-style interactions for nested collections. The main shell remains profile-centered, while edit flows use draft structs plus focused collection editors for:
- `requires` entries
- rule list management
- rule destinations
- rule tags

Why this choice:
- The existing TUI is a single-screen shell; a mode-based extension fits it better than a full route hierarchy.
- List-like fields are awkward to edit inline, so small local overlays provide structure without turning the UI into a text editor.
- This borrows the useful part of `cc-switch-cli`'s approach, namely "summary row -> focused list editor -> row editor", without importing its broader system complexity.

Alternatives considered:
- Inline-edit every field directly in the detail pane. Rejected because list fields and nested rules become hard to understand and error-prone.
- Introduce a generalized route/overlay framework before editing. Rejected because the first feature does not justify that scale.

### 4. Use draft snapshots for dirty detection instead of manual dirty flags

Profile editing state will keep a normalized snapshot of the initial draft and compare it with the current draft when the operator tries to leave edit mode. If they differ, the TUI will show a save/discard/cancel confirmation.

Why this choice:
- The draft can change through text edits, toggles, add/remove actions, rule reordering, and nested collection editors.
- Snapshot comparison is simpler and less fragile than remembering to set a `dirty` flag in every mutation path.
- This matches the strongest lesson from `cc-switch-cli`'s unsaved-edit design without requiring its full architecture.

Alternatives considered:
- A mutable `dirty: bool` flag. Rejected because it is easy to miss mutation paths once nested editors are introduced.

### 5. Save by validate-then-atomic-rewrite, and only reload live config after successful validation

Saving from the TUI will follow this sequence:
1. apply the profile draft into the raw editable config
2. serialize that raw config to YAML text
3. validate the candidate config through the existing config loader/validator logic
4. atomically rewrite the config file if validation succeeds
5. reload raw and validated config into the TUI

If validation or write fails, the TUI will keep the draft open and surface the error.

Why this choice:
- It prevents half-valid config from becoming the active in-memory state.
- It reuses the existing config validation semantics instead of inventing a second editor-only validator.
- Atomic rewrite matches the current safety posture elsewhere in the tool.

Alternatives considered:
- Mutate the in-memory config first and delay filesystem write. Rejected because the TUI could diverge from the true source of truth.
- Save first and validate on next refresh. Rejected because it allows a broken config to hit disk silently.

## Risks / Trade-offs

- [Normalized rewrites remove comments and original formatting] -> Mitigation: document that behavior clearly and keep it scoped to TUI saves; do not imply comment preservation.
- [Editable raw model and runtime validated model can drift] -> Mitigation: make the editable raw model reuse the same schema fields and route all save validation through the existing config validator.
- [Editing nested rule collections can still feel clunky in a small TUI] -> Mitigation: use focused collection overlays with compact summaries instead of trying to inline every nested field.
- [Profile renames and deletes can disrupt selection state] -> Mitigation: explicitly re-anchor selection after save/delete and require confirmation before destructive removal.
- [Validation errors from whole-config checks may feel indirect when editing one profile] -> Mitigation: surface the original validation error and keep the operator in the draft so they can correct it immediately.

## Migration Plan

1. Add OpenSpec deltas for profile editing behavior and TUI edit-mode behavior.
2. Refactor the config module to expose a serializable raw/editable config representation and a shared validate-from-raw path.
3. Add profile draft state, edit mode, collection overlays, and unsaved-change confirms to the TUI.
4. Add atomic save/reload behavior and deletion/create flows.
5. Add regression coverage for raw config save behavior, TUI editing flows, validation failure handling, and normalized rewrite expectations.
6. Update README and operator guidance to explain that TUI profile edits rewrite YAML in normalized form and do not preserve comments.

## Open Questions

- None for the first proposal. The main scope boundary is already intentional: profile-only editing with normalized rewrite semantics.
