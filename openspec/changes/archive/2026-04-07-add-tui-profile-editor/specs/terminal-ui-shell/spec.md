## ADDED Requirements

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
