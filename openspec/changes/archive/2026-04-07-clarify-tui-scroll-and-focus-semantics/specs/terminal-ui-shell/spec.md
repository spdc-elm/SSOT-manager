## MODIFIED Requirements

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
