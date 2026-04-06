## 1. Bootstrap And Config Resolution

- [x] 1.1 Scaffold the `SSOT-manager/` CLI workspace with command entry points for `config`, `profile`, and `undo`
- [x] 1.2 Implement YAML config loading and validation for `version`, `source_root`, named `profiles`, and `symlink`-only rules
- [x] 1.3 Add fixture-based tests for valid config parsing, unknown profiles, disabled rules, and unsupported modes

## 2. Planning And Apply Engine

- [x] 2.1 Implement deterministic profile resolution and source asset discovery from ordered rules and `select` matchers
- [x] 2.2 Implement reconciliation logic that classifies targets as `create`, `update`, `remove`, `skip`, `warning`, or `danger`
- [x] 2.3 Implement `ssot profile plan <name>` and `ssot profile apply <name>` with danger blocking and post-apply symlink verification
- [x] 2.4 Add filesystem integration tests for missing targets, matching symlinks, and unmanaged collisions

## 3. Managed State, Doctor, And Undo

- [x] 3.1 Implement local state storage for managed records and the last-successful-apply journal
- [x] 3.2 Implement `ssot profile doctor <name>` to report broken symlinks, missing managed targets, and stale managed drift
- [x] 3.3 Implement `ssot undo` to revert only the most recent successful recorded apply
- [x] 3.4 Add end-to-end tests covering apply, manual drift, doctor output, and undo recovery

## 4. Dogfood And Documentation

- [x] 4.1 Create a sample config for this repo and exercise the MVP commands against safe test destinations
- [x] 4.2 Document the MVP CLI workflow, safety model, state directory behavior, and explicit non-goals in `SSOT-manager/README.md`
