## MODIFIED Requirements

### Requirement: YAML config is validated before any profile action
The system SHALL parse a YAML configuration that defines `version`, `source_root`, named `profiles`, and an optional named `compositions` map for prompt compilation. Each composition MUST declare ordered source inputs, a generated output path under `source_root`, and a supported built-in renderer configuration. A profile MAY declare an ordered `requires` list of composition names, and the system MUST reject the config if any required composition name is undefined. The system MUST reject the config before planning, applying, or compiling if required fields are missing, if a rule declares an unknown materialization mode, or if a composition declares an unsupported renderer or an output path outside `source_root`. A profile MAY declare its own `source_root`, which overrides the top-level `source_root` for that profile only. Supported rule modes MUST include `symlink`, `copy`, and `hardlink`.

#### Scenario: Valid config with compositions passes validation
- **WHEN** a config contains `version`, `source_root`, one named profile, and one named composition whose output path stays under `source_root`
- **THEN** the system accepts the config for subsequent profile resolution and prompt compilation

#### Scenario: Unknown mode is rejected
- **WHEN** a rule declares a mode other than `symlink`, `copy`, or `hardlink`
- **THEN** the system fails validation and reports that the mode is unknown

#### Scenario: Composition output outside source root is rejected
- **WHEN** a composition declares an output path that resolves outside the configured `source_root`
- **THEN** the system fails validation before any compile or profile action runs

#### Scenario: Unknown required composition is rejected
- **WHEN** a profile declares a required composition name that is not defined in the config
- **THEN** the system fails validation before any compile or profile action runs
