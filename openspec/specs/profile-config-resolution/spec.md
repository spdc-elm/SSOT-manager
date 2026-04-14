# profile-config-resolution Specification

## Purpose
Define how the SSOT manager validates YAML config and resolves a named profile into deterministic sync intents.
## Requirements
### Requirement: YAML config is validated before any profile action
The system SHALL parse a YAML configuration that defines `version`, `source_root`, named `profiles`, and an optional named `compositions` map for prompt compilation. Each composition MUST declare ordered source inputs, a generated output path under `source_root`, and a supported built-in renderer configuration. A profile MAY declare an ordered `requires` list of composition names, and each rule MAY declare an optional ordered `ignore` list of glob patterns that are interpreted relative to the matched asset root. The system MUST reject the config if any required composition name is undefined. The system MUST reject the config before planning, applying, or compiling if required fields are missing, if a rule declares an unknown materialization mode, if a rule declares an invalid ignore glob, or if a composition declares an unsupported renderer or an output path outside `source_root`. A profile MAY declare its own `source_root`, which overrides the top-level `source_root` for that profile only. Supported rule modes MUST include `symlink`, `copy`, and `hardlink`.

#### Scenario: Valid config with compositions passes validation
- **WHEN** a config contains `version`, `source_root`, one named profile, and one named composition whose output path stays under `source_root`
- **THEN** the system accepts the config for subsequent profile resolution and prompt compilation

#### Scenario: Unknown mode is rejected
- **WHEN** a rule declares a mode other than `symlink`, `copy`, or `hardlink`
- **THEN** the system fails validation and reports that the mode is unknown

#### Scenario: Invalid ignore glob is rejected
- **WHEN** a rule declares an `ignore` pattern that is not a valid glob expression
- **THEN** the system fails validation before any compile or profile action runs

#### Scenario: Composition output outside source root is rejected
- **WHEN** a composition declares an output path that resolves outside the configured `source_root`
- **THEN** the system fails validation before any compile or profile action runs

#### Scenario: Unknown required composition is rejected
- **WHEN** a profile declares a required composition name that is not defined in the config
- **THEN** the system fails validation before any compile or profile action runs

### Requirement: Named profiles resolve into deterministic sync intents
The system SHALL resolve a requested profile by reading its rules in declaration order, ignoring rules explicitly marked `enabled: false`, expanding each `select` matcher against that profile's effective source root, and pairing every matched asset with each destination path in `to`. Each resolved sync intent MUST preserve the rule's declared materialization mode and ignore glob list.

#### Scenario: Rules resolve in declaration order
- **WHEN** a profile contains multiple enabled rules with overlapping destination roots
- **THEN** the system emits sync intents in the same rule order as the config so the resulting plan is deterministic

#### Scenario: Disabled rules are ignored
- **WHEN** a profile rule is marked `enabled: false`
- **THEN** the system excludes that rule from the resolved sync intents

#### Scenario: Profile source root overrides the global default
- **WHEN** a profile declares its own `source_root`
- **THEN** the system expands that profile's `select` globs relative to the profile-level source root instead of the top-level default

#### Scenario: Resolved intents preserve mode and ignore policy
- **WHEN** a profile contains enabled rules with different materialization modes and ignore glob lists
- **THEN** each resolved intent reports the same mode and ignore glob list as the rule that produced it

### Requirement: Unknown profiles fail before reconciliation
The system MUST reject a profile command if the requested profile name is not defined in the config.

#### Scenario: Missing profile is reported
- **WHEN** the operator runs a profile command with a profile name that does not exist
- **THEN** the system stops before planning and reports the profile as unknown
