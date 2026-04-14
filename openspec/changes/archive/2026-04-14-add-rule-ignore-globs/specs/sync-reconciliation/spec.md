## MODIFIED Requirements

### Requirement: The planner classifies target changes explicitly
The system SHALL compare the desired sync intents for a profile against the current filesystem and classify each target as `create`, `update`, `remove`, `skip`, `warning`, or `danger`. The comparison MUST evaluate whether the current target already matches the desired source asset under the intent's declared materialization mode. When a directory asset is evaluated under a rule with `ignore` globs, the comparison MUST exclude descendants whose relative paths match those globs from both the desired source tree and the current target tree before deciding whether the target already matches.

#### Scenario: Missing target becomes create
- **WHEN** a desired managed target path does not exist in the filesystem
- **THEN** the generated plan marks that target as `create`

#### Scenario: Matching materialized target becomes skip
- **WHEN** a target already exists and matches the desired source asset under its declared materialization mode
- **THEN** the generated plan marks that target as `skip`

#### Scenario: Ignored target-only metadata does not force an update
- **WHEN** a directory target differs from the desired source tree only by descendants whose relative paths match the rule's `ignore` globs
- **THEN** the generated plan marks that target as `skip`

### Requirement: Apply executes only safe planned actions and verifies results
The system SHALL execute filesystem mutations only from the computed plan, refuse to apply `danger` actions by default, and verify after mutation that each changed target matches the expected managed source asset under the declared materialization mode. For directory assets, `copy` MUST create an equivalent directory tree with copied file contents, and `hardlink` MUST create an equivalent directory tree whose leaf files are hardlinked to the corresponding source files. When a rule declares `ignore` globs for a directory asset, the runtime MUST exclude ignore-matched descendants from the desired materialized tree and from post-apply verification. The system SHALL support an explicit force-with-backup apply path that may replace forceable `danger` targets only after recording a restorable backup of the unmanaged content.

#### Scenario: Safe plan is applied and verified
- **WHEN** a profile plan contains only `create`, `update`, `remove`, `skip`, or `warning` actions
- **THEN** the system applies the allowed mutations and verifies that each changed target matches the expected source asset for `symlink`, `copy`, or `hardlink`

#### Scenario: Hardlink mode materializes a directory tree
- **WHEN** a rule with `mode: hardlink` matches a source directory
- **THEN** the applied target becomes a directory tree with the same relative entries and hardlinked leaf files

#### Scenario: Ignored source descendants are not materialized
- **WHEN** a directory rule declares `ignore` globs and a source descendant's relative path matches one of them
- **THEN** `copy` and `hardlink` materialization omit that descendant from the resulting managed target tree

#### Scenario: Dangerous plan is refused by ordinary apply
- **WHEN** a profile plan contains one or more `danger` actions
- **THEN** the system aborts the ordinary apply before mutating those targets and reports the blocking dangers

#### Scenario: Force-with-backup applies a forceable unmanaged collision
- **WHEN** a profile plan contains only forceable `danger` collisions and the operator explicitly chooses the backup-overwrite apply path
- **THEN** the system records backups for those unmanaged targets, replaces them with the desired managed materialization, and verifies the resulting targets
