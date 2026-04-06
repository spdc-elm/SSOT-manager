## ADDED Requirements

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
The system SHALL let the operator trigger existing reconcile actions from the terminal UI while preserving the same safety rules as the CLI.

#### Scenario: TUI apply respects danger blocking
- **WHEN** the operator triggers apply for a profile whose plan contains danger actions
- **THEN** the system blocks the mutation and surfaces the same dangerous outcome classification instead of applying changes

#### Scenario: TUI undo uses the existing last-apply journal
- **WHEN** the operator triggers undo from the terminal UI after a successful apply
- **THEN** the system reuses the existing undo behavior and reports the recorded rollback result
