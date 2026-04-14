## 1. Config And Intent Model

- [x] 1.1 Extend editable/runtime rule models to accept optional rule-level `ignore` glob lists and reject invalid patterns during config validation
- [x] 1.2 Preserve rule ignore globs in resolved sync intents and expose them through inspection output where relevant

## 2. Reconcile, State, And Undo Semantics

- [x] 2.1 Update directory comparison and materialization logic so `copy` and `hardlink` rules exclude ignore-matched descendants from desired tree evaluation
- [x] 2.2 Update plan/apply verification and doctor drift detection to honor rule ignore globs for managed directory targets
- [x] 2.3 Extend last-apply journal and undo safety checks so recorded ignore policy is preserved and ignored descendants do not block undo

## 3. Tests And Documentation

- [x] 3.1 Add regression coverage for config validation, plan/doctor behavior, and apply/undo flows involving ignored directory descendants
- [x] 3.2 Update README and config schema references to document `rule.ignore` semantics and examples
- [x] 3.3 Update `Skills/ssot-manager-config` guidance to recommend platform-specific ignore patterns for `copy` and `hardlink` directory rules without introducing runtime defaults
