## MODIFIED Requirements

### Requirement: Unmanaged collisions are surfaced as danger by default
The system MUST treat an existing target that is not recorded as managed and would need to be replaced as a `danger` action instead of overwriting it silently. The system MAY additionally classify a danger as forceable when the collision is an unmanaged file, directory, or symlink that can be replaced only through an explicit backup-overwrite apply path.

#### Scenario: Existing unmanaged file blocks ordinary apply
- **WHEN** a desired target path already contains an unmanaged regular file
- **THEN** the generated plan marks that target as `danger`

#### Scenario: Forceable unmanaged collision remains danger in the plan
- **WHEN** a desired target path contains an unmanaged file, directory, or symlink that the manager can replace via backup overwrite
- **THEN** the generated plan still marks that target as `danger` rather than downgrading it to a safe action

### Requirement: Apply executes only safe planned actions and verifies results
The system SHALL execute filesystem mutations only from the computed plan, refuse to apply `danger` actions by default, and verify after mutation that each changed target matches the expected managed source asset under the declared materialization mode. For directory assets, `copy` MUST create an equivalent directory tree with copied file contents, and `hardlink` MUST create an equivalent directory tree whose leaf files are hardlinked to the corresponding source files. The system SHALL support an explicit force-with-backup apply path that may replace forceable `danger` targets only after recording a restorable backup of the unmanaged content.

#### Scenario: Safe plan is applied and verified
- **WHEN** a profile plan contains only `create`, `update`, `remove`, `skip`, or `warning` actions
- **THEN** the system applies the allowed mutations and verifies that each changed target matches the expected source asset for `symlink`, `copy`, or `hardlink`

#### Scenario: Hardlink mode materializes a directory tree
- **WHEN** a rule with `mode: hardlink` matches a source directory
- **THEN** the applied target becomes a directory tree with the same relative entries and hardlinked leaf files

#### Scenario: Dangerous plan is refused by ordinary apply
- **WHEN** a profile plan contains one or more `danger` actions
- **THEN** the system aborts the ordinary apply before mutating those targets and reports the blocking dangers

#### Scenario: Force-with-backup applies a forceable unmanaged collision
- **WHEN** a profile plan contains only forceable `danger` collisions and the operator explicitly chooses the backup-overwrite apply path
- **THEN** the system records backups for those unmanaged targets, replaces them with the desired managed materialization, and verifies the resulting targets
