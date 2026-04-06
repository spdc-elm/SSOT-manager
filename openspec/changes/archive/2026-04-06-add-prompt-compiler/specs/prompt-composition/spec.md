## ADDED Requirements

### Requirement: Operator can declare deterministic prompt composition recipes
The system SHALL accept a named prompt composition configuration that declares ordered source inputs, a generated output path, a built-in renderer configuration, and optional declared variables. Each composition output path MUST resolve under the configured `source_root`, and the system MUST reject compositions whose output paths escape that root.

#### Scenario: Valid composition is accepted
- **WHEN** the operator defines a composition that reads `Agents/assistant.md` and `USER.md` in order and writes to a generated path under `source_root`
- **THEN** the system accepts the composition as a valid prompt recipe

#### Scenario: Output path escapes source root
- **WHEN** a composition declares an output path outside the configured `source_root`
- **THEN** the system rejects the configuration before any compile action runs

### Requirement: Built-in prompt rendering preserves declared order and wrapper structure
The system SHALL compile a prompt composition by reading declared inputs in order, applying the configured per-input wrapper and outer wrapper, and resolving declared variable placeholders inside wrapper templates and static literal fragments. The system MUST fail the compile if a built-in template references an undefined declared variable.

#### Scenario: Assistant and user documents are wrapped deterministically
- **WHEN** a composition declares `Agents/assistant.md` followed by `USER.md` and configures per-input XML wrappers plus an outer wrapper
- **THEN** the compiled output contains the assistant content first, the user content second, and all configured wrappers in deterministic order

#### Scenario: Undefined variable blocks compilation
- **WHEN** a built-in wrapper template references a variable that is not declared for the composition
- **THEN** the system fails the compile and reports the missing variable instead of emitting partial output

### Requirement: Unsupported script renderers are rejected in the first version
The system MUST reject any composition that declares a renderer kind other than the built-in renderer set supported by the first version, including a reserved `script` renderer kind.

#### Scenario: Script renderer is rejected
- **WHEN** a composition declares `renderer: script`
- **THEN** the system rejects the configuration or compile request and reports that scripted renderers are not yet supported

### Requirement: Composition prerequisite status is available to profile and TUI flows
The system SHALL expose a deterministic readiness check for each composition that can report whether the generated output is ready, missing, or stale with respect to the composition's declared inputs and recipe definition.

#### Scenario: Missing generated output is reported as missing
- **WHEN** a composition's generated output file does not exist at its declared path
- **THEN** the readiness check reports that composition as missing

#### Scenario: Outdated generated output is reported as stale
- **WHEN** a composition's generated output no longer reflects its current declared inputs or recipe definition
- **THEN** the readiness check reports that composition as stale
