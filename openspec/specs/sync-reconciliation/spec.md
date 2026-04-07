# sync-reconciliation Specification

## Purpose
Define how the SSOT manager compares desired sync state to the live filesystem and applies safe, verified changes.
## Requirements
### Requirement: The planner classifies target changes explicitly
The system SHALL compare the desired sync intents for a profile against the current filesystem and classify each target as `create`, `update`, `remove`, `skip`, `warning`, or `danger`. The comparison MUST evaluate whether the current target already matches the desired source asset under the intent's declared materialization mode.

#### Scenario: Missing target becomes create
- **WHEN** a desired managed target path does not exist in the filesystem
- **THEN** the generated plan marks that target as `create`

#### Scenario: Matching materialized target becomes skip
- **WHEN** a target already exists and matches the desired source asset under its declared materialization mode
- **THEN** the generated plan marks that target as `skip`

### Requirement: Unmanaged collisions are surfaced as danger by default
The system MUST treat an existing target that is not recorded as managed and would need to be replaced as a `danger` action instead of overwriting it silently. The system MAY additionally classify a danger as forceable when the collision is an unmanaged file, directory, or symlink that can be replaced only through an explicit backup-overwrite apply path.

#### Scenario: Existing unmanaged file blocks ordinary apply
- **WHEN** a desired target path already contains an unmanaged regular file
- **THEN** the generated plan marks that target as `danger`

#### Scenario: Forceable unmanaged collision remains danger in the plan
- **WHEN** a desired target path contains an unmanaged file, directory, or symlink that the manager can replace via backup overwrite
- **THEN** the generated plan still marks that target as `danger` rather than downgrading it to a safe action

### Requirement: Planner blocks self-referential source and target overlap
The system SHALL detect when a target's effective materialization path overlaps the desired source path, including overlap introduced by symlinked ancestor directories, and it SHALL classify that target as a non-forceable `danger`.

#### Scenario: Parent symlink makes the target resolve back into the source tree
- **WHEN** a desired target path appears separate textually but resolves into the source asset path or source subtree because one of its existing ancestor directories is a symlink
- **THEN** the generated plan marks that target as `danger`
- **AND** that danger is not forceable through the backup-overwrite apply path

#### Scenario: Ordinary apply refuses self-referential overlap
- **WHEN** a profile plan contains a self-referential source/target overlap danger
- **THEN** ordinary apply aborts before mutating the target
- **AND** the reported reason identifies the source/target overlap rather than a generic unmanaged collision

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

### Requirement: Profile reconcile checks required prompt compositions before filesystem planning
The system SHALL evaluate every composition named in a profile's `requires` list before normal filesystem reconcile begins. Missing or stale required compositions MUST surface as blocking prerequisite issues instead of degrading into ordinary missing-asset discovery warnings.

#### Scenario: Missing required composition blocks profile plan
- **WHEN** a profile declares a required composition whose generated output is missing
- **THEN** the system reports a blocking prerequisite issue for that composition before treating its generated asset path as an ordinary source discovery problem

#### Scenario: Stale required composition blocks profile apply
- **WHEN** a profile declares a required composition whose generated output is stale
- **THEN** the system refuses profile apply until that composition has been rebuilt
