# prompt-compiler-cli Specification

## Purpose
TBD - created by archiving change add-prompt-compiler. Update Purpose after archive.
## Requirements
### Requirement: Operator can inspect configured prompt compositions from the CLI
The system SHALL provide prompt compiler inspection commands that list configured compositions and show the effective definition of a named composition without mutating generated outputs or sync targets.

#### Scenario: Prompt list reports configured compositions
- **WHEN** the operator runs the prompt listing command against a valid config with multiple named compositions
- **THEN** the system outputs the configured composition names in deterministic order without mutating state

#### Scenario: Prompt show reports effective recipe details
- **WHEN** the operator runs the prompt show command for a valid composition
- **THEN** the system reports the composition name, ordered inputs, output path, declared variables, and built-in wrapper configuration

### Requirement: Operator can preview and build compiled prompt outputs from the CLI
The system SHALL provide compiler commands that preview the compiled output for a named composition and materialize the generated output file for a named composition. Preview MUST not write files, and build MUST write the compiled output to the declared generated path, creating parent directories when needed.

#### Scenario: Preview shows compiled output without writing files
- **WHEN** the operator runs the prompt preview command for a valid composition
- **THEN** the system emits the compiled prompt text and leaves the generated output path unchanged

#### Scenario: Build materializes the generated prompt file
- **WHEN** the operator runs the prompt build command for a valid composition
- **THEN** the system writes the compiled prompt text to the declared generated output path and reports the materialized file location

