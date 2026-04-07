## 1. TUI Module Split

- [x] 1.1 Split `src/tui.rs` into a small `src/tui/` module tree with separate files for public wiring, state, input handling, rendering, and tests
- [x] 1.2 Preserve current behavior during the split, including the already-landed detail page scrolling and editor list viewport fixes
- [x] 1.3 Keep TUI unit-style tests near the TUI module so private render/state helpers remain testable without widening visibility

## 2. Main-Shell Focus State

- [x] 2.1 Add an explicit non-editing shell focus state for profile browsing versus detail inspection
- [x] 2.2 Wire `Enter`, `Esc`, and `h` to move between browse mode and detail-focus mode without losing the selected profile or active detail tab
- [x] 2.3 Rebind `j`/`k` and `Up`/`Down` so they move profiles in browse mode and scroll the detail pane in detail-focus mode

## 3. Detail Overflow Affordances

- [x] 3.1 Refine right-pane scroll state so it clamps correctly across profile changes, tab changes, refreshes, and focus transitions
- [x] 3.2 Render a visible overflow cue or position indicator for long right-pane detail content
- [x] 3.3 Update footer or in-pane guidance so the current browse/detail key semantics remain discoverable

## 4. Verification And Documentation

- [x] 4.1 Add TUI-focused tests for pane-focus transitions, mode-dependent key handling, and long-detail scrolling behavior
- [x] 4.2 Add rendering tests that assert overflow indicators appear only when the detail pane actually exceeds the viewport
- [x] 4.3 Update README navigation docs to describe browse mode, detail focus mode, and the long-detail reading flow
