## MODIFIED Requirements

### Requirement: Successful applies persist a last-apply journal for rollback
The system SHALL store a durable last-successful-apply journal that captures the filesystem mutations needed to undo the apply. The recorded journal state MUST distinguish enough filesystem detail to detect whether a copied or hardlinked target changed after apply before undo proceeds. For managed directory targets whose rule declared `ignore` globs, the journal MUST also preserve the ignore comparison policy needed to re-evaluate post-apply safety during undo. When a force-with-backup apply replaces previously unmanaged content, the journal MUST also record the manager-owned backup artifact needed to restore that overwritten target.

#### Scenario: Journal is available for undo
- **WHEN** a profile apply completes successfully
- **THEN** the system writes a last-apply journal that can be used by `undo`

#### Scenario: Forced overwrite records backup metadata
- **WHEN** the operator force-applies a plan that replaces an unmanaged file, directory, or symlink
- **THEN** the journal records the backup artifact needed to restore that overwritten target

#### Scenario: Journal preserves ignore policy for directory targets
- **WHEN** a successful apply manages a directory target under a rule that declares `ignore` globs
- **THEN** the last-apply journal preserves the ignore comparison policy for that target

### Requirement: Doctor reports stale and broken managed targets
The system SHALL compare managed records against the live filesystem and report at least broken symlinks, missing managed targets, and targets whose current state no longer matches the recorded managed expectation for their materialization mode. When a managed directory target is governed by a rule that declares `ignore` globs, doctor drift checks MUST exclude descendants whose relative paths match those globs before deciding whether the target is stale managed drift.

#### Scenario: Broken symlink is reported
- **WHEN** a managed target is a symlink whose source no longer exists
- **THEN** the doctor output reports that target as broken

#### Scenario: Copied target drift is reported
- **WHEN** a managed target recorded with `mode: copy` no longer matches the source asset contents
- **THEN** the doctor output reports that target as stale managed drift

#### Scenario: Hardlinked target drift is reported
- **WHEN** a managed target recorded with `mode: hardlink` no longer matches the source asset via the expected hardlink relationship
- **THEN** the doctor output reports that target as stale managed drift

#### Scenario: Ignored target metadata is not reported as drift
- **WHEN** a managed directory target differs from its expected tree only by descendants whose relative paths match the rule's `ignore` globs
- **THEN** the doctor output does not report that target as stale managed drift

### Requirement: Undo only reverts the last successful recorded apply
The system MUST limit `undo` to the most recent successful apply journal and MUST refuse to modify targets that are not covered by that recorded journal. It MUST also refuse undo when a recorded target no longer matches the journal's post-apply state, including copied or hardlinked targets that were edited after apply. For managed directory targets recorded with `ignore` globs, undo MUST evaluate post-apply state using the ignore policy stored in the journal rather than treating ignored descendants as modifications. When the last successful apply journal contains backup-overwrite entries, undo MUST restore the recorded unmanaged backup content for those targets instead of leaving them missing.

#### Scenario: Undo reverts last apply
- **WHEN** the operator runs `undo` immediately after a successful apply
- **THEN** the system reverts the recorded mutations from that last apply journal

#### Scenario: Modified copied target blocks undo
- **WHEN** a copied target recorded in the last successful apply journal has been edited after apply
- **THEN** `undo` refuses to modify that target because it no longer matches the recorded post-apply state

#### Scenario: Ignored metadata does not block undo
- **WHEN** a managed directory target differs from the recorded post-apply tree only by descendants that match the journal's recorded `ignore` globs
- **THEN** `undo` still treats that target as matching the recorded post-apply state

#### Scenario: Unrecorded target is not touched by undo
- **WHEN** a target path was not included in the last successful apply journal
- **THEN** the system leaves that target unchanged during `undo`

#### Scenario: Undo restores overwritten unmanaged content from backup
- **WHEN** the last successful apply journal contains a backup-overwrite entry and the current target still matches the recorded post-apply state
- **THEN** undo restores the original unmanaged content from the recorded backup artifact
