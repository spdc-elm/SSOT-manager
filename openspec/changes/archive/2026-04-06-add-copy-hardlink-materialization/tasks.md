## 1. Config And Reconcile Foundations

- [x] 1.1 Extend `MaterializationMode` parsing, formatting, and resolved intents to accept `copy` and `hardlink`
- [x] 1.2 Replace symlink-only target matching and materialization with mode-aware plan/apply helpers for files and directory trees

## 2. State Safety And Operator Surface

- [x] 2.1 Extend journal/state snapshot logic so doctor and undo can validate copied and hardlinked targets safely
- [x] 2.2 Update CLI/TUI inspection output and README text to reflect multi-mode materialization behavior

## 3. Verification

- [x] 3.1 Add regression coverage for config validation and inspection of copy/hardlink modes
- [x] 3.2 Add integration coverage for apply, doctor, and undo using copied and hardlinked targets
