## MODIFIED Requirements

### Requirement: Successful applies persist managed ownership records
The system SHALL record managed ownership for every target it creates, updates, or removes during a successful apply, including the profile name, source asset path, target path, materialization mode, and apply timestamp.

#### Scenario: Managed records are written after apply
- **WHEN** a profile apply completes successfully
- **THEN** the local state directory stores managed records for the targets that were reconciled by that apply

### Requirement: Successful applies persist a last-apply journal for rollback
The system SHALL store a durable last-successful-apply journal that captures the filesystem mutations needed to undo the apply. The recorded journal state MUST distinguish enough filesystem detail to detect whether a copied or hardlinked target changed after apply before undo proceeds.

#### Scenario: Journal is available for undo
- **WHEN** a profile apply completes successfully
- **THEN** the system writes a last-apply journal that can be used by `undo`

### Requirement: Doctor reports stale and broken managed targets
The system SHALL compare managed records against the live filesystem and report at least broken symlinks, missing managed targets, and targets whose current state no longer matches the recorded managed expectation for their materialization mode.

#### Scenario: Broken symlink is reported
- **WHEN** a managed target is a symlink whose source no longer exists
- **THEN** the doctor output reports that target as broken

#### Scenario: Copied target drift is reported
- **WHEN** a managed target recorded with `mode: copy` no longer matches the source asset contents
- **THEN** the doctor output reports that target as stale managed drift

#### Scenario: Hardlinked target drift is reported
- **WHEN** a managed target recorded with `mode: hardlink` no longer matches the source asset via the expected hardlink relationship
- **THEN** the doctor output reports that target as stale managed drift

### Requirement: Undo only reverts the last successful recorded apply
The system MUST limit `undo` to the most recent successful apply journal and MUST refuse to modify targets that are not covered by that recorded journal. It MUST also refuse undo when a recorded target no longer matches the journal's post-apply state, including copied or hardlinked targets that were edited after apply.

#### Scenario: Undo reverts last apply
- **WHEN** the operator runs `undo` immediately after a successful apply
- **THEN** the system reverts the recorded mutations from that last apply journal

#### Scenario: Modified copied target blocks undo
- **WHEN** a copied target recorded in the last successful apply journal has been edited after apply
- **THEN** `undo` refuses to modify that target because it no longer matches the recorded post-apply state

#### Scenario: Unrecorded target is not touched by undo
- **WHEN** a target path was not included in the last successful apply journal
- **THEN** the system leaves that target unchanged during `undo`
