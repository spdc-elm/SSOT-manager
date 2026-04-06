## MODIFIED Requirements

### Requirement: YAML config is validated before any profile action
The system SHALL parse a YAML configuration that defines `version`, `source_root`, and named `profiles`, and it MUST reject the config before planning or applying if required fields are missing or a rule declares an unknown materialization mode. A profile MAY declare its own `source_root`, which overrides the top-level `source_root` for that profile only. Supported rule modes MUST include `symlink`, `copy`, and `hardlink`.

#### Scenario: Valid config passes validation
- **WHEN** a config contains `version`, `source_root`, one named profile, and rules that use `mode: symlink`, `mode: copy`, or `mode: hardlink`
- **THEN** the system accepts the config for subsequent profile resolution

#### Scenario: Unknown mode is rejected
- **WHEN** a rule declares a mode other than `symlink`, `copy`, or `hardlink`
- **THEN** the system fails validation and reports that the mode is unknown

### Requirement: Named profiles resolve into deterministic sync intents
The system SHALL resolve a requested profile by reading its rules in declaration order, ignoring rules explicitly marked `enabled: false`, expanding each `select` matcher against that profile's effective source root, and pairing every matched asset with each destination path in `to`. Each resolved sync intent MUST preserve the rule's declared materialization mode.

#### Scenario: Rules resolve in declaration order
- **WHEN** a profile contains multiple enabled rules with overlapping destination roots
- **THEN** the system emits sync intents in the same rule order as the config so the resulting plan is deterministic

#### Scenario: Disabled rules are ignored
- **WHEN** a profile rule is marked `enabled: false`
- **THEN** the system excludes that rule from the resolved sync intents

#### Scenario: Profile source root overrides the global default
- **WHEN** a profile declares its own `source_root`
- **THEN** the system expands that profile's `select` globs relative to the profile-level source root instead of the top-level default

#### Scenario: Resolved intents preserve mode
- **WHEN** a profile contains enabled rules with different materialization modes
- **THEN** each resolved intent reports the same mode as the rule that produced it
