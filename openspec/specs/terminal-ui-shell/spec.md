# terminal-ui-shell Specification

## Purpose
TBD - created by archiving change add-thin-terminal-ui. Update Purpose after archive.
## Requirements
### Requirement: Operator can browse profiles in a terminal UI
The system SHALL provide a terminal UI entry point that lists configured profiles and lets the operator select one active profile for inspection.

#### Scenario: TUI opens with profile navigation
- **WHEN** the operator launches the terminal UI against a valid config
- **THEN** the system displays a navigable list of configured profiles and highlights one active profile

### Requirement: TUI shows structured inspection views for the active profile
The system SHALL display structured views for the selected profile using the same inspection data model as the CLI inspection commands.

#### Scenario: TUI renders show, plan, and doctor data
- **WHEN** the operator selects a profile in the terminal UI
- **THEN** the system can display the profile's effective definition, current plan state, and doctor results without shelling out to parse CLI text output

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

### Requirement: TUI shows prompt prerequisites for the selected profile
The system SHALL extend the terminal UI so the operator can inspect the selected profile's required prompt compositions, their current prerequisite status, and their generated output paths through shared in-process compiler and profile inspection data.

#### Scenario: TUI shows prompt prerequisite details for a profile
- **WHEN** the operator opens the terminal UI against a config whose selected profile declares required prompt compositions
- **THEN** the system displays those required compositions and their current prerequisite state without shelling out to CLI text commands

### Requirement: TUI can compile required prompt compositions before sync actions
The system SHALL let the operator trigger compilation of the selected profile's required prompt compositions from the terminal UI and then continue to existing profile plan/apply flows using shared library APIs. If prompt compilation fails, the TUI MUST surface the compile error and MUST NOT proceed as if sync had succeeded.

#### Scenario: Compile dependencies succeeds before profile sync
- **WHEN** the operator triggers prompt compilation for the selected profile's required compositions from the terminal UI
- **THEN** the system materializes the generated prompt outputs and keeps the operator in a state where normal profile plan/apply actions can use those outputs

#### Scenario: Compile failure blocks false success
- **WHEN** prompt compilation fails from the terminal UI
- **THEN** the system reports the compile error and does not report a successful sync/apply outcome

