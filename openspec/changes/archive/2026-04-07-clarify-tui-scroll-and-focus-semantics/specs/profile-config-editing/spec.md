## MODIFIED Requirements

### Requirement: Operator can manage nested profile collections without leaving the TUI
The system SHALL let the operator add, edit, reorder, and remove profile-local collection entries from the TUI, including `requires` items, rules, rule destinations, and rule tags.

#### Scenario: Add and remove profile rules
- **WHEN** the operator edits a profile's rule list in the TUI
- **THEN** the system allows adding a new rule draft and removing an existing rule without leaving the profile edit flow
- **AND** the active rule remains visible inside the popup when the rule list is taller than the available viewport

#### Scenario: Edit rule destinations as a collection
- **WHEN** the operator edits a rule's destination list in the TUI
- **THEN** the system allows managing the ordered `to` entries as structured list items instead of one freeform scalar field

#### Scenario: Long collection popups advertise overflow state
- **WHEN** a collection editor popup for `requires`, rules, rule destinations, or rule tags is taller than the visible popup viewport
- **THEN** the system displays a visible overflow cue, such as a scrollbar or scrollbar-like position indicator, inside that popup
- **AND** the cue updates as the operator moves through the collection
