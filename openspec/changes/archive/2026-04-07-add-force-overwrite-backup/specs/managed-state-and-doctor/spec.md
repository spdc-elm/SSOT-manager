## MODIFIED Requirements

### Requirement: Successful applies persist a last-apply journal for rollback
The system SHALL store a durable last-successful-apply journal that captures the filesystem mutations needed to undo the apply. The recorded journal state MUST distinguish enough filesystem detail to detect whether a copied or hardlinked target changed after apply before undo proceeds. When a force-with-backup apply replaces previously unmanaged content, the journal MUST also record the manager-owned backup artifact needed to restore that overwritten target.

#### Scenario: Journal is available for undo
- **WHEN** a profile apply completes successfully
- **THEN** the system writes a last-apply journal that can be used by `undo`

#### Scenario: Forced overwrite records backup metadata
- **WHEN** the operator force-applies a plan that replaces an unmanaged file, directory, or symlink
- **THEN** the journal records the backup artifact needed to restore that overwritten target

### Requirement: Undo only reverts the last successful recorded apply
The system MUST limit `undo` to the most recent successful apply journal and MUST refuse to modify targets that are not covered by that recorded journal. It MUST also refuse undo when a recorded target no longer matches the journal's post-apply state, including copied or hardlinked targets that were edited after apply. When the last successful apply journal contains backup-overwrite entries, undo MUST restore the recorded unmanaged backup content for those targets instead of leaving them missing.

#### Scenario: Undo reverts last apply
- **WHEN** the operator runs `undo` immediately after a successful apply
- **THEN** the system reverts the recorded mutations from that last apply journal

#### Scenario: Modified copied target blocks undo
- **WHEN** a copied target recorded in the last successful apply journal has been edited after apply
- **THEN** `undo` refuses to modify that target because it no longer matches the recorded post-apply state

#### Scenario: Unrecorded target is not touched by undo
- **WHEN** a target path was not included in the last successful apply journal
- **THEN** the system leaves that target unchanged during `undo`

#### Scenario: Undo restores overwritten unmanaged content from backup
- **WHEN** the last successful apply journal contains a backup-overwrite entry and the current target still matches the recorded post-apply state
- **THEN** undo restores the original unmanaged content from the recorded backup artifact
