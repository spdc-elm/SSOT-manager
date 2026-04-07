## Context

The TUI recently gained explicit detail focus, visible scroll affordances, and overflow handling in both inspection and editing flows. Those behaviors now exist in code and tests, but the main specs still leave two important edges underspecified:

- the exact meaning of `h` while detail inspection is focused
- the expectation that long editing popups should visibly advertise overflow rather than only silently scrolling

This change is intentionally spec-centric. The implementation already behaves the way we want; the gap is that future work could regress these details without clearly violating the documented product contract.

## Goals / Non-Goals

**Goals:**
- Make `h` in focused inspection mode a stable, testable navigation rule rather than an implicit implementation detail.
- Clarify that long inspection detail and long editing collections both need visible overflow affordances.
- Keep the spec language at the product-behavior level so different render implementations remain possible.

**Non-Goals:**
- Reworking the current TUI implementation or changing keybindings again.
- Standardizing on one exact scrollbar glyph set, color, or helper shape.
- Turning implementation-level reuse decisions into product requirements.

## Decisions

### 1. Specify `h` in detail focus as a staged navigation rule

The terminal UI spec will state that `h` behaves differently depending on the active inspection tab while detail focus is active:

- on `Doctor` or `Plan`, `h` moves to the previous inspection tab and keeps detail focus active
- on `Show`, `h` returns to profile browsing

Why this choice:
- It matches the now-implemented behavior and the user's expected leftward navigation model.
- It keeps `h` semantically aligned with "move left" before using it as "leave the focused right pane".
- It prevents future regressions where `h` would always eject operators from focused reading.

Alternatives considered:
- Keep `h` underspecified and rely on implementation comments. Rejected because this is user-visible navigation behavior.
- Require `h` to always leave detail focus. Rejected because it breaks the natural tab-to-tab leftward progression inside focused inspection.

### 2. Specify visible overflow affordances as a product requirement, not a specific widget contract

Both inspection detail and long editing collection popups will be specified as needing visible overflow affordances. The wording will allow a scrollbar or scrollbar-like indicator rather than locking the system to one rendering technique.

Why this choice:
- The durable contract is that long content must look scrollable, not that one exact glyph pattern must be used forever.
- It preserves room for future visual refinement without reopening the behavioral requirement.
- It brings the editing surface up to the same legibility standard already expected from the inspection pane.

Alternatives considered:
- Require a literal scrollbar in spec text. Rejected because it overconstrains presentation when the core need is visible overflow state.
- Keep editing popups at "selected item stays visible" only. Rejected because that still leaves long lists under-signaled to operators.

## Risks / Trade-offs

- [Spec language could become too implementation-shaped] -> Mitigation: require visible overflow affordances while avoiding helper-level or glyph-level prescriptions.
- [Two related behaviors might be split across specs inconsistently] -> Mitigation: keep inspection semantics in `terminal-ui-shell` and editing popup semantics in `profile-config-editing`, each attached to the capability that owns the user-facing behavior.
- [This could look redundant because code already implements it] -> Mitigation: treat the change as contract hardening so future refactors have an explicit baseline.

## Migration Plan

1. Add delta spec updates for `terminal-ui-shell` and `profile-config-editing`.
2. Sync the clarified requirements into the main specs during archive.
3. No code migration is required unless a later audit finds implementation drift.

## Open Questions

- None. The current implementation already demonstrates the intended behavior; this change exists to make that behavior durable in the specs.
