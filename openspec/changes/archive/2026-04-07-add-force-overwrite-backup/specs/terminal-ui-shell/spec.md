## MODIFIED Requirements

### Requirement: TUI can invoke existing safe reconcile actions
The system SHALL let the operator trigger existing reconcile actions from the terminal UI while preserving the same safety rules as the CLI. If the selected profile plan contains only forceable dangers, the TUI SHALL support an explicit repeated-apply confirmation flow that force-applies with backup on the second confirmation instead of applying on the first keypress.

#### Scenario: TUI apply respects danger blocking
- **WHEN** the operator triggers apply for a profile whose plan contains danger actions that are not forceable
- **THEN** the system blocks the mutation and surfaces the same dangerous outcome classification instead of applying changes

#### Scenario: TUI second apply confirms force-with-backup
- **WHEN** the operator triggers apply for a profile whose plan contains only forceable dangers and then triggers apply again without changing context
- **THEN** the second apply performs the backup-overwrite force apply rather than blocking as an ordinary danger

#### Scenario: TUI undo uses the existing last-apply journal
- **WHEN** the operator triggers undo from the terminal UI after a successful apply
- **THEN** the system reuses the existing undo behavior and reports the recorded rollback result
