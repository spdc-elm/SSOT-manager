# SSOT-manager Draft

## 1. Goal

Build a focused SSOT manager for personal AI flavour assets.

The system should:
- treat one repo as the only editable source of truth
- manage sync from source assets to one or more destinations
- make current state visible and explainable
- keep sync deterministic and trustworthy
- support safe preview and rollback-oriented operation

This is **not** meant to be a general environment manager. It should stay focused on SSOT asset management.

## 2. Core Principles

1. AI can author config, but the manager must execute deterministically.
2. Visibility is first-class: `show`, `plan`, `doctor`, `explain` matter as much as `apply`.
3. Managed boundaries must be explicit. The tool should know what it owns.
4. Dangerous operations must be surfaced, not hidden.
5. Default behavior should be conservative around unmanaged files.
6. The tool should remain useful even if the config is partly AI-authored and frequently edited.

## 3. Product Scope

### In Scope

- personal prompt/skill/agent/spec-style assets
- one source root
- one or more named profiles
- profile composition
- mechanical sync to destinations
- state tracking
- preview, apply, verify, warning, undo

### Out of Scope for V1

- provider account config
- env var management
- MCP/process orchestration
- arbitrary runtime shell automation
- LLM-dependent apply logic

## 4. Asset-First Model

The system is asset-first, not app-first.

The user should be able to think in terms of:
- what asset exists
- where it should be enabled
- how it should be materialized

Not:
- what each app happens to want today

Platform-specific paths are adapter concerns, not the core mental model.

## 5. Minimal Core Model

The minimal model should be:

- `source_root`
- `profiles`
- `rules`

Each rule should answer:
- what to select
- where to sync it
- how to materialize it

### Proposed Entities

#### `source_root`

Absolute path to the SSOT asset repository.

#### `profile`

A named ruleset. Examples:
- `skill-global`
- `skill-kg`
- `skill-all`

A profile may include other profiles, but composition must be explicit and predictable.

#### `rule`

A rule contains:
- `select`
- `to`
- `mode`
- optional `enabled`
- optional `tags`
- optional `note`

#### `mode`

V1 supports only:
- `symlink`
- `hardlink`
- `copy`

This keeps the engine mechanical and auditable.

## 6. Config Format

Primary config format: YAML.

Reasoning:
- nested rule objects and lists are natural in YAML
- easy for humans and AI to author
- easier to express one selector to many destinations
- good fit for examples and skill-style docs

### Example Shape

```yaml
version: 1

source_root: /path/to/personal-harness-management

profiles:
  skill-global:
    rules:
      - select: Skills/*
        to:
          - ~/.agents/skills/
        mode: symlink

      - select: Agents/assistant.md
        to:
          - ~/.codex/AGENTS.md
        mode: symlink

  skill-kg:
    rules:
      - select: KG_Local/Skills/*
        to:
          - /path/to/AI-KnowledgeGarden/.agent/skills/
          - /path/to/AI-KnowledgeGarden/.claude/skills/
        mode: symlink

  skill-all:
    include:
      - skill-global
      - skill-kg
```

## 7. Profile Composition

Profile composition is useful, but dangerous if underspecified.

### Allowed in V1

- `include` other profiles
- no cycles
- deterministic merge order
- explicit conflict reporting

### Must Be Defined Clearly

- include order
- override precedence
- duplicate rule handling
- behavior when two profiles target the same destination differently

If this is not nailed down, the system becomes hard to trust.

## 8. Managed Boundary

The tool should explicitly distinguish:
- managed target
- unmanaged target
- stale managed target
- broken target

This is critical for safe apply behavior.

Default policy:
- never silently overwrite unmanaged files
- never silently delete unmanaged files
- mark risky collisions as dangerous

## 9. Engine Responsibilities

The engine should follow a reconcile model:

1. read config
2. resolve profile composition
3. discover source assets
4. compute desired target state
5. inspect current target state
6. diff desired vs actual
7. emit plan
8. optionally apply
9. verify result
10. record journal/state

## 10. State Directory

The manager is allowed to keep a local state directory.

Recommended default:
- `~/.local/state/ssot-manager/` on XDG-friendly systems

Fallbacks can be decided later for non-XDG environments.

### State Should Store

- last apply journal
- managed records
- undo metadata
- cached target snapshots
- warning history if useful

### Why State Matters

Without local state:
- `undo` is fragile
- managed/unmanaged boundary is unclear
- drift detection becomes weaker
- trust drops sharply

## 11. Plan / Apply / Undo

### `plan`

Dry-run style output with explicit action classes:
- `create`
- `update`
- `remove`
- `skip`
- `danger`
- `warning`

### `apply`

Requirements:
- show summary before mutation
- block or require explicit confirmation on dangerous operations
- record a journal before changes
- verify after changes

### `undo`

V1 undo can be intentionally modest:
- undo only the last successful apply
- only for changes the manager recorded
- prioritize symlink/hardlink/copy reversibility over full transactional guarantees

This is enough to create trust without overengineering a distributed transaction system.

## 12. Warning System

Warnings should be structured, not ad hoc strings.

Suggested warning/error classes:
- `source_missing`
- `asset_not_found`
- `target_missing`
- `broken_symlink`
- `stale_managed_link`
- `managed_drift`
- `unmanaged_collision`
- `profile_conflict`
- `profile_cycle`
- `dangerous_overwrite`

This matters because renames and moves in the source repo will happen.

The tool should make those failures visible early through normal inspection commands, not only at apply time.

## 13. CLI First

The product should be CLI-first.

Suggested V1 commands:
- `ssot profile list`
- `ssot profile show <name>`
- `ssot profile plan <name>`
- `ssot profile apply <name>`
- `ssot profile doctor <name>`
- `ssot profile explain <name>`
- `ssot asset list`
- `ssot asset status`
- `ssot config validate`
- `ssot undo`

The TUI can come later as a front-end over the same engine.

## 14. TUI Role

The TUI should not become a second source of truth.

Its job should be:
- browse assets
- browse profiles
- inspect warnings
- preview plans
- toggle enable/disable state in config
- help edit config safely

The TUI should be a convenience layer over the deterministic core, not a separate orchestration path.

## 15. Transform / Preprocess Hooks

There is a real future need for transformation-like behavior:
- render a template before copying
- adapt one prompt format to another host format
- preprocess metadata before emission

But this is risky because it can turn a declarative sync engine into a hidden scripting runtime.

### V1 Decision

Do **not** support arbitrary user-defined hooks in V1.

Instead:
- reserve an extension point in the design
- keep the core model mechanical
- consider future controlled transforms as built-in plugins or a limited interface

This preserves trust in apply semantics.

## 16. Failure Modes

If this project fails in practice, it will likely fail for one of these reasons:

1. It grows from a sync manager into a vague personal config platform.
2. Profile composition becomes too implicit to reason about.
3. Managed ownership is unclear, so the tool becomes too timid or too dangerous.
4. Undo is weak, so users stop trusting apply.
5. Visibility is poor, so users fall back to manual `ln -s`.
6. Transform features become too script-like and make results non-deterministic.

The design should actively resist these failure modes.

## 17. V1 Success Criteria

V1 is successful if it can reliably do the following:

- read one YAML config
- resolve one or more profiles
- show desired vs actual state clearly
- warn about broken or stale targets
- apply only mechanical sync actions
- refuse unsafe overwrites by default
- undo the last apply well enough to recover from common mistakes

If V1 can do this well, later flexibility will be worth adding.

## 18. Immediate Next Design Questions

These are still open:

1. exact config schema
2. profile merge semantics
3. state file/journal schema
4. how assets are identified long-term: path-only vs optional stable metadata ID
5. exact output format for `plan` and `doctor`

## 19. Current Position

The current direction is:

- asset-first
- YAML config
- profile-based rules
- deterministic reconcile engine
- explicit warning system
- local state directory
- CLI first, TUI later
- no arbitrary hook execution in V1

That is a sufficiently sharp V1 direction without overcommitting to a full platform.
