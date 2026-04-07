# SSOT Manager Authoring Patterns

## Preferred Default: Assets-First

Use assets-first when the user thinks in terms of source assets such as skills, prompts, notes, or agents that should be synced into one or more installation surfaces.

Assets-first is a family of authoring patterns, not one fixed profile layout.

### Recommended Mode: Source-Assets-First

Use this as the default when one bundle should distribute the same source assets to multiple consumers.

### Pattern

- One profile per applyable source bundle
- One rule per source asset
- Put all destinations for that source asset on the rule's `to` list
- Keep generated prompt outputs as their own rules in the same profile when they belong to the same bundle

### Example

```yaml
version: 1
source_root: ../personal-harness-management

profiles:
  skill-global:
    requires:
      - codex-agent
    rules:
      - select: Skills/brainstorming
        to:
          - ~/.codex/skills/
          - ~/.config/opencode/skills/
          - ~/.gemini/antigravity/skills/
        mode: symlink
      - select: Skills/prompt-iteration
        to:
          - ~/.codex/skills/
          - ~/.config/opencode/skills/
          - ~/.gemini/antigravity/skills/
        mode: symlink
        enabled: false
      - select: build/prompts/codex/AGENTS.generated.md
        to:
          - ~/.codex/AGENTS.md
        mode: symlink
```

### Why This Fits Current Runtime

- It maps directly onto the current `profile -> rules -> assets` model.
- It keeps toggles attached to real source assets, not wildcard expansion results.
- It lets one rule fan out to multiple consumers without inventing new semantics.
- It matches cases where the author thinks "this skill should go to these three tools" rather than "what does tool X receive?"

### Alternative Mode: Surface-Bundle Assets-First

Use this when the user still thinks in source assets, but wants separate apply/undo units per destination surface.

### Pattern

- One profile per installation surface or bundle
- One rule per asset when that asset needs an independent toggle
- Separate prompt-sync profiles from skills-sync profiles when they target different consumer files

### Example

```yaml
profiles:
  codex-skills:
    rules:
      - select: Skills/brainstorming
        to:
          - ~/.codex/skills/
        mode: symlink
      - select: Skills/prompt-iteration
        to:
          - ~/.codex/skills/
        mode: symlink
        enabled: false

  opencode-skills:
    rules:
      - select: Skills/brainstorming
        to:
          - ~/.config/opencode/skills/
        mode: symlink

  codex-agent-prompt:
    requires:
      - codex-agent
    rules:
      - select: build/prompts/codex/AGENTS.generated.md
        to:
          - ~/.codex/AGENTS.md
        mode: symlink
```

### Tradeoff

- This keeps apply/undo surfaces isolated.
- It is still assets-first, but less centered on "one source bundle" than the recommended mode.
- It avoids pretending wildcard expansion has first-class identity.

## Project-First

Use project-first when the user primarily thinks in terms of a destination environment and wants one profile per project bundle.

### Pattern

- One profile per target environment
- Rules grouped by what the project needs
- Good when the main question is "what should project X receive?"

### Example

```yaml
profiles:
  project1:
    requires:
      - codex-agent
    rules:
      - select: Skills/brainstorming
        to:
          - /work/project1/.codex/skills/
        mode: symlink
      - select: Skills/ctf-helper
        to:
          - /work/project1/.codex/skills/
        mode: symlink
      - select: build/prompts/codex/AGENTS.generated.md
        to:
          - /work/project1/AGENTS.md
        mode: symlink
```

## Flat YAML Principle

Prefer explicit flat YAML as the runtime truth.

- Good: one rule per independently toggled asset
- Good: helper scripts that scaffold flat rules from an asset directory
- Bad: extending the runtime config format before the authoring pain is real

## Anti-Patterns

### Single wildcard rule for independently toggled assets

Avoid:

```yaml
- select: Skills/*
  to:
    - ~/.codex/skills/
  mode: symlink
```

when the user wants per-skill enable/disable.

### Overlapping profiles expecting inheritance

Avoid assuming `codex-skills` automatically layers on top of `skill-global`, or `project1-skills` on top of `global-skills`. Current runtime treats those as separate owners when targets overlap.

### Absolute `select`

Avoid absolute filesystem paths in `select`. `select` is matched relative to effective `source_root`.

### Mixed generated and hand-edited truth without a rule

If generated output is only a starting point, say so. Do not silently regenerate over manual TUI edits.
