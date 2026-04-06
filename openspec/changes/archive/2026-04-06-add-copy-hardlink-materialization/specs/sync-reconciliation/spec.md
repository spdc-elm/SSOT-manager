## MODIFIED Requirements

### Requirement: The planner classifies target changes explicitly
The system SHALL compare the desired sync intents for a profile against the current filesystem and classify each target as `create`, `update`, `remove`, `skip`, `warning`, or `danger`. The comparison MUST evaluate whether the current target already matches the desired source asset under the intent's declared materialization mode.

#### Scenario: Missing target becomes create
- **WHEN** a desired managed target path does not exist in the filesystem
- **THEN** the generated plan marks that target as `create`

#### Scenario: Matching materialized target becomes skip
- **WHEN** a target already exists and matches the desired source asset under its declared materialization mode
- **THEN** the generated plan marks that target as `skip`

### Requirement: Unmanaged collisions are surfaced as danger by default
The system MUST treat an existing target that is not recorded as managed and would need to be replaced as a `danger` action instead of overwriting it silently.

#### Scenario: Existing unmanaged file blocks apply
- **WHEN** a desired target path already contains an unmanaged regular file
- **THEN** the generated plan marks that target as `danger`

### Requirement: Apply executes only safe planned actions and verifies results
The system SHALL execute filesystem mutations only from the computed plan, refuse to apply `danger` actions by default, and verify after mutation that each changed target matches the expected managed source asset under the declared materialization mode. For directory assets, `copy` MUST create an equivalent directory tree with copied file contents, and `hardlink` MUST create an equivalent directory tree whose leaf files are hardlinked to the corresponding source files.

#### Scenario: Safe plan is applied and verified
- **WHEN** a profile plan contains only `create`, `update`, `remove`, `skip`, or `warning` actions
- **THEN** the system applies the allowed mutations and verifies that each changed target matches the expected source asset for `symlink`, `copy`, or `hardlink`

#### Scenario: Hardlink mode materializes a directory tree
- **WHEN** a rule with `mode: hardlink` matches a source directory
- **THEN** the applied target becomes a directory tree with the same relative entries and hardlinked leaf files

#### Scenario: Dangerous plan is refused
- **WHEN** a profile plan contains one or more `danger` actions
- **THEN** the system aborts the apply before mutating those targets and reports the blocking dangers
