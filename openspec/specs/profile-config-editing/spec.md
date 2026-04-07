# profile-config-editing Specification

## Purpose
Define how the SSOT manager exposes profile configuration editing through the terminal UI and persists those edits back to normalized YAML.

## Requirements
### Requirement: Operator can edit profile definitions through structured TUI controls
The system SHALL provide a profile editing surface in the terminal UI that lets the operator edit profile configuration through structured controls instead of hand-editing YAML text. The editable profile data MUST include the profile name, optional profile `source_root`, ordered `requires` entries, and ordered rule definitions with each rule's `select`, `to`, `mode`, `enabled`, `tags`, and `note` fields.

#### Scenario: Edit an existing profile in the TUI
- **WHEN** the operator opens the edit surface for an existing profile
- **THEN** the system displays the current profile data as editable structured fields rather than raw YAML text

#### Scenario: Create a new profile in the TUI
- **WHEN** the operator creates a new profile from the TUI
- **THEN** the system opens an editable structured draft for a new named profile definition

### Requirement: Operator can manage nested profile collections without leaving the TUI
The system SHALL let the operator add, edit, reorder, and remove profile-local collection entries from the TUI, including `requires` items, rules, rule destinations, and rule tags.

#### Scenario: Add and remove profile rules
- **WHEN** the operator edits a profile's rule list in the TUI
- **THEN** the system allows adding a new rule draft and removing an existing rule without leaving the profile edit flow
- **AND** the active rule remains visible inside the popup when the rule list is taller than the available viewport

#### Scenario: Edit rule destinations as a collection
- **WHEN** the operator edits a rule's destination list in the TUI
- **THEN** the system allows managing the ordered `to` entries as structured list items instead of one freeform scalar field

### Requirement: Nested profile editors return to their parent context
The system SHALL treat nested editors in the profile editing flow as explicit layers. Leaving or completing a nested editor MUST return the operator to that editor's immediate parent context rather than closing the entire profile editor.

#### Scenario: Escape closes one nested layer at a time
- **WHEN** the operator presses `Esc` from a nested collection or field editor inside profile editing
- **THEN** the system returns to that nested editor's parent context instead of leaving the entire profile editor

#### Scenario: Saving a nested entry returns to the parent editor
- **WHEN** the operator confirms a nested field or collection entry edit from the TUI
- **THEN** the system returns to the parent collection or rule editor with the updated value selected

### Requirement: Rule enabled state is controllable and visible from the rule list
The system SHALL expose each rule's enabled or disabled state directly in the rule list and allow the operator to toggle the selected rule's enabled state from that list without first opening the rule detail editor.

#### Scenario: Rule list shows enabled state clearly
- **WHEN** the operator opens the rule list for a profile in the TUI
- **THEN** the system displays each rule with a visible enabled or disabled status indicator

#### Scenario: Rule list toggles enabled state directly
- **WHEN** the operator toggles the selected rule from the rule list in the TUI
- **THEN** the system updates that rule's enabled state in the current profile draft without requiring a separate rule detail view

### Requirement: Path-oriented text inputs support inline completion
The system SHALL provide inline path completion for path-oriented profile editing fields such as profile `source_root` and rule destinations. Completion candidates MUST be shown in the active input popup, and repeated completion triggers MUST allow cycling through the current match set.

#### Scenario: Path completion candidates are shown in the popup
- **WHEN** the operator triggers completion while editing a path-oriented field in the TUI
- **THEN** the system shows the matching path candidates inside the active input popup rather than only in a global status area

#### Scenario: Repeated completion cycles through current candidates
- **WHEN** multiple path candidates match the current input in a path-oriented field
- **THEN** repeated completion triggers cycle through those candidates in the active popup

### Requirement: TUI profile saves validate and atomically rewrite the config file
The system SHALL save TUI profile edits by validating the candidate config against the same config rules used by normal profile operations and then atomically rewriting the YAML config file in normalized form. If validation or write fails, the system MUST keep the edit draft open and MUST NOT replace the active config used by the TUI.

#### Scenario: Successful save rewrites normalized YAML
- **WHEN** the operator saves a valid profile edit from the TUI
- **THEN** the system atomically rewrites the config file in normalized YAML form and reloads the updated config into the TUI

#### Scenario: Invalid save keeps the draft open
- **WHEN** the operator attempts to save a profile edit whose candidate config fails validation
- **THEN** the system reports the validation error, leaves the edit draft open, and does not replace the currently active config

### Requirement: TUI profile editing makes unsaved changes explicit
The system SHALL detect unsaved profile draft changes and require an explicit operator choice to save, discard, or cancel before leaving the edit surface.

#### Scenario: Dirty draft prompts before exit
- **WHEN** the operator attempts to leave a profile edit surface with unsaved changes
- **THEN** the system prompts the operator to save, discard, or cancel instead of closing immediately

#### Scenario: Clean draft exits without confirmation
- **WHEN** the operator leaves a profile edit surface whose draft matches its initial snapshot
- **THEN** the system exits the edit surface without showing an unsaved-change prompt

### Requirement: Operator can delete a profile definition from the TUI
The system SHALL let the operator delete an existing profile definition from the TUI through an explicit confirmation flow.

#### Scenario: Delete confirmed profile
- **WHEN** the operator confirms deletion of an existing profile from the TUI
- **THEN** the system removes that profile definition from the config, rewrites the config file, and returns to the profile list without selecting the deleted profile

#### Scenario: Delete canceled profile
- **WHEN** the operator declines the delete confirmation for an existing profile
- **THEN** the system keeps the profile definition unchanged and remains in the current editing context
