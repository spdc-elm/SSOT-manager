## ADDED Requirements

### Requirement: Profile reconcile checks required prompt compositions before filesystem planning
The system SHALL evaluate every composition named in a profile's `requires` list before normal filesystem reconcile begins. Missing or stale required compositions MUST surface as blocking prerequisite issues instead of degrading into ordinary missing-asset discovery warnings.

#### Scenario: Missing required composition blocks profile plan
- **WHEN** a profile declares a required composition whose generated output is missing
- **THEN** the system reports a blocking prerequisite issue for that composition before treating its generated asset path as an ordinary source discovery problem

#### Scenario: Stale required composition blocks profile apply
- **WHEN** a profile declares a required composition whose generated output is stale
- **THEN** the system refuses profile apply until that composition has been rebuilt
