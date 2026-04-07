## ADDED Requirements

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
