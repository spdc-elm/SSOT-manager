# terminal-ui-shell Specification

## Purpose
TBD - created by archiving change add-thin-terminal-ui. Update Purpose after archive.
## Requirements
### Requirement: Operator can browse profiles in a terminal UI
The system SHALL provide a terminal UI entry point that lists configured profiles and lets the operator select one active profile for inspection.

#### Scenario: TUI opens with profile navigation
- **WHEN** the operator launches the terminal UI against a valid config
- **THEN** the system displays a navigable list of configured profiles and highlights one active profile

### Requirement: Operator can focus the inspection detail pane without leaving the main shell
The system SHALL let the operator switch between profile-list browsing and focused detail inspection within the main terminal UI shell while keeping the same selected profile and active inspection view.

#### Scenario: Enter focuses the current detail pane
- **WHEN** the operator is in the main inspection shell with a selected profile and presses `Enter`
- **THEN** the system moves navigation focus from the profile list to the currently visible detail pane
- **AND** the selected profile and active inspection tab remain unchanged

#### Scenario: Escape returns to profile browsing
- **WHEN** the operator is inspecting a focused detail pane and presses `Esc`
- **THEN** the system returns navigation focus to the profile list instead of leaving the TUI or clearing the current preview
- **AND** the selected profile and active inspection tab remain unchanged

#### Scenario: H moves to the previous inspection tab before leaving detail focus
- **WHEN** the operator is inspecting a focused detail pane on the `Doctor` or `Plan` tab and presses `h`
- **THEN** the system switches to the previous inspection tab
- **AND** the detail pane remains focused

#### Scenario: H leaves detail focus from the leftmost inspection tab
- **WHEN** the operator is inspecting a focused detail pane on the `Show` tab and presses `h`
- **THEN** the system returns navigation focus to the profile list
- **AND** the selected profile remains unchanged

#### Scenario: Focus changes the meaning of vertical navigation keys
- **WHEN** the operator presses `j`/`k` or `Up`/`Down` in the main shell
- **THEN** those keys move between profiles while the profile list is focused
- **AND** those keys scroll the detail viewport while the detail pane is focused

### Requirement: TUI shows structured inspection views for the active profile
The system SHALL display structured views for the selected profile using the same inspection data model as the CLI inspection commands, and it SHALL make long inspection output visibly navigable inside the right-hand detail pane.

#### Scenario: TUI renders show, plan, and doctor data
- **WHEN** the operator selects a profile in the terminal UI
- **THEN** the system can display the profile's effective definition, current plan state, and doctor results without shelling out to parse CLI text output
- **AND** the TUI can present the selected profile's `source_root` separately so repeated source-side paths may be shown relative to that root for readability
- **AND** the inspection detail pane remains navigable when the rendered content is taller than the current terminal viewport

#### Scenario: Scrollable detail advertises overflow state
- **WHEN** the active inspection content is taller than the visible detail viewport
- **THEN** the system displays a visible overflow cue, such as a scrollbar or scrollbar-like position indicator, in the detail pane
- **AND** that cue updates as the operator scrolls through the detail content

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

### Requirement: TUI can transition between profile inspection and profile editing
The system SHALL extend the terminal UI with a profile editing entry point that starts from the selected profile in the existing profile-centered shell and returns to the inspection surface after save, discard, or delete outcomes.

#### Scenario: Enter edit mode from the selected profile
- **WHEN** the operator triggers profile editing from the terminal UI while a profile is selected
- **THEN** the system opens the editing surface for that selected profile without leaving the TUI process

#### Scenario: Save returns to inspection with updated profile data
- **WHEN** the operator saves profile edits successfully from the TUI
- **THEN** the system returns to the profile-centered inspection shell with the updated profile selected

### Requirement: TUI profile editing remains compatible with existing inspection and reconcile flows
The system SHALL keep existing profile inspection, prompt prerequisite inspection, and reconcile actions available outside the edit surface, and it MUST require the operator to leave profile editing before using compile, apply, undo, or refresh actions.

#### Scenario: Reconcile actions stay outside edit mode
- **WHEN** the operator is currently editing a profile in the TUI
- **THEN** the system does not perform compile, apply, undo, or refresh actions until the operator exits the edit surface

#### Scenario: Inspection resumes after discarding edits
- **WHEN** the operator discards a profile draft and exits the edit surface
- **THEN** the system returns to the normal inspection shell without mutating config-backed inspection data
