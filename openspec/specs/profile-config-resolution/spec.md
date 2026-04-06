# profile-config-resolution Specification

## Purpose
Define how the SSOT manager validates YAML config and resolves a named profile into deterministic sync intents.
## Requirements
### Requirement: YAML config is validated before any profile action
The system SHALL parse a YAML configuration that defines `version`, `source_root`, and named `profiles`, and it MUST reject the config before planning or applying if required fields are missing or a rule declares an unsupported materialization mode. A profile MAY declare its own `source_root`, which overrides the top-level `source_root` for that profile only.

#### Scenario: Valid config passes validation
- **WHEN** a config contains `version`, `source_root`, one named profile, and rules that use `mode: symlink`
- **THEN** the system accepts the config for subsequent profile resolution

#### Scenario: Unsupported mode is rejected
- **WHEN** a rule declares `mode: copy` or `mode: hardlink` in the MVP
- **THEN** the system fails validation and reports that only `symlink` is supported

### Requirement: Named profiles resolve into deterministic sync intents
The system SHALL resolve a requested profile by reading its rules in declaration order, ignoring rules explicitly marked `enabled: false`, expanding each `select` matcher against that profile's effective source root, and pairing every matched asset with each destination path in `to`.

#### Scenario: Rules resolve in declaration order
- **WHEN** a profile contains multiple enabled rules with overlapping destination roots
- **THEN** the system emits sync intents in the same rule order as the config so the resulting plan is deterministic

#### Scenario: Disabled rules are ignored
- **WHEN** a profile rule is marked `enabled: false`
- **THEN** the system excludes that rule from the resolved sync intents

#### Scenario: Profile source root overrides the global default
- **WHEN** a profile declares its own `source_root`
- **THEN** the system expands that profile's `select` globs relative to the profile-level source root instead of the top-level default

### Requirement: Unknown profiles fail before reconciliation
The system MUST reject a profile command if the requested profile name is not defined in the config.

#### Scenario: Missing profile is reported
- **WHEN** the operator runs a profile command with a profile name that does not exist
- **THEN** the system stops before planning and reports the profile as unknown
