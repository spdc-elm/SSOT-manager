## ADDED Requirements

### Requirement: Operator can list configured profiles
The system SHALL provide a profile listing command that enumerates the profiles defined in the loaded config in deterministic order.

#### Scenario: Profiles are listed in config order
- **WHEN** the operator runs the profile listing command against a valid config with multiple named profiles
- **THEN** the system outputs each profile name in deterministic order without mutating state

### Requirement: Operator can inspect the effective definition of a profile
The system SHALL provide a profile inspection command that shows a profile's effective source root and declared rules without consulting live filesystem state.

#### Scenario: Profile show reports effective source root and rules
- **WHEN** the operator runs the profile show command for a valid profile
- **THEN** the system reports the profile name, effective source root, and ordered rules including each rule's selector, destinations, mode, and enabled state

### Requirement: Operator can explain a profile's current resolved outcome
The system SHALL provide a profile explanation command that combines profile resolution diagnostics with the current reconcile plan for that profile.

#### Scenario: Profile explain includes diagnostics and plan summary
- **WHEN** the operator runs the profile explain command for a valid profile
- **THEN** the system reports any profile resolution diagnostics, the resolved intents, and the current plan action summary for that profile

### Requirement: Inspection commands support machine-readable output
The system SHALL support machine-readable JSON output for the profile listing, profile show, and profile explain commands.

#### Scenario: JSON output preserves inspection fields
- **WHEN** the operator runs one of the profile inspection commands with the JSON output flag
- **THEN** the system emits structured JSON containing the same inspection fields as the corresponding command's data model
