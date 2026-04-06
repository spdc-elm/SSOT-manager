## 1. TUI Foundation

- [ ] 1.1 Land the CLI inspection change first and wire the TUI to the shared inspection and reconcile library interfaces
- [ ] 1.2 Add the terminal UI dependency stack and create a `ssot tui` entry point in the existing binary

## 2. Profile-Centered Screens

- [ ] 2.1 Build a navigable profile list and detail view that can render show, plan, and doctor data for the active profile
- [ ] 2.2 Add refresh and mode-switching behavior so the UI can update inspection data without restarting

## 3. Safe Actions And Validation

- [ ] 3.1 Wire `apply` and `undo` into the TUI using the existing engine behavior and blocked-danger semantics
- [ ] 3.2 Dogfood the TUI against the sample config and add focused tests for navigation, rendering, and safe action handling
