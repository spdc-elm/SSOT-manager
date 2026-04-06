# sync-reconciliation Specification

## Purpose
Define how the SSOT manager compares desired sync state to the live filesystem and applies safe, verified changes.
## Requirements
### Requirement: The planner classifies target changes explicitly
The system SHALL compare the desired sync intents for a profile against the current filesystem and classify each target as `create`, `update`, `remove`, `skip`, `warning`, or `danger`.

#### Scenario: Missing target becomes create
- **WHEN** a desired managed target path does not exist in the filesystem
- **THEN** the generated plan marks that target as `create`

#### Scenario: Matching symlink becomes skip
- **WHEN** a target already exists as a symlink to the desired source asset
- **THEN** the generated plan marks that target as `skip`

### Requirement: Unmanaged collisions are surfaced as danger by default
The system MUST treat an existing target that is not recorded as managed and would need to be replaced as a `danger` action instead of overwriting it silently.

#### Scenario: Existing unmanaged file blocks apply
- **WHEN** a desired target path already contains an unmanaged regular file
- **THEN** the generated plan marks that target as `danger`

### Requirement: Apply executes only safe planned actions and verifies results
The system SHALL execute filesystem mutations only from the computed plan, refuse to apply `danger` actions by default, and verify after mutation that each changed target matches the expected managed symlink.

#### Scenario: Safe plan is applied and verified
- **WHEN** a profile plan contains only `create`, `update`, `remove`, `skip`, or `warning` actions
- **THEN** the system applies the allowed mutations and verifies that each changed target points to the expected source asset

#### Scenario: Dangerous plan is refused
- **WHEN** a profile plan contains one or more `danger` actions
- **THEN** the system aborts the apply before mutating those targets and reports the blocking dangers
