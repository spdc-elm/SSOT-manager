## ADDED Requirements

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
