## Context

The repo already treats one workspace as the editable source of truth for prompt, skill, and agent assets, but sync is still partly script-shaped and hard to inspect. The draft defines the right product direction, yet the full V1 surface includes multiple materialization modes, profile composition, rich visibility commands, undo, warnings, and future transform hooks.

The MVP should prove a narrower claim first: for personal AI flavour assets, a conservative CLI can resolve a named profile, compute a trustworthy sync plan, apply mechanical filesystem changes, and explain or undo what it changed. If that loop is not solid, the rest of the product will not be trusted.

## Goals / Non-Goals

**Goals:**
- Load one YAML config rooted in a single source repository.
- Resolve one named profile into deterministic sync actions.
- Support `plan`, `apply`, `doctor`, and `undo` around managed destinations.
- Record enough local state to explain ownership, detect drift, and revert the last successful apply.
- Default to conservative behavior when targets are unmanaged or dangerous.

**Non-Goals:**
- Profile `include` semantics, merge precedence, or conflict resolution in the MVP.
- `hardlink` and `copy` materialization modes.
- Arbitrary hooks, transforms, or template rendering.
- TUI workflows or interactive config editing.
- Long-term stable asset IDs beyond source-relative paths.

## Decisions

### 1. Keep the config model but constrain execution to one profile and one mode

The config will keep the draft's top-level shape: `version`, `source_root`, `profiles`, and `rules`. Each rule still declares `select`, `to`, `mode`, and optional metadata, but the MVP validator will only accept `mode: symlink`.

Why this choice:
- It preserves the intended asset-first mental model.
- It avoids redesign when `copy` or `hardlink` are added later.
- Symlinks are the most inspectable and reversible materialization for this repo's current use case.

Alternatives considered:
- Support all three modes immediately. Rejected because copy drift and hardlink semantics add state and undo complexity before the core trust loop is proven.
- Collapse config into a profile-less rule list. Rejected because named profiles are already part of the intended operator model.

### 2. Structure the engine as resolve -> plan -> apply -> verify -> journal

The engine will separate five steps:
1. Parse and validate YAML config.
2. Resolve one named profile into ordered rules and matching source assets.
3. Reconcile desired targets against the live filesystem to produce a typed plan.
4. Execute only safe actions, then verify resulting links.
5. Persist managed records and a last-apply journal.

Why this choice:
- It makes dry-run and mutation share the same decision path.
- It keeps output explainable because every applied action originates from a planned action.
- It creates a clean boundary for `doctor` and `undo`.

Alternatives considered:
- Mutate directly during traversal. Rejected because it is harder to explain, test, and recover.

### 3. Use explicit local state to track ownership and rollback

The MVP will maintain a local state directory, expected under `~/.local/state/ssot-manager/`, with at least:
- a managed-records manifest keyed by target path
- a last-successful-apply journal with before/after details for each mutation

Managed records should store enough information to answer:
- which profile created the target
- which source path the target should point to
- what mode was applied
- when it was last verified or updated

Why this choice:
- The filesystem alone cannot reliably distinguish managed drift from unmanaged collisions.
- Undo requires a durable record of what changed.
- Doctor needs a source of truth for stale or broken managed targets.

Alternatives considered:
- No local state, derive everything from the current filesystem. Rejected because undo becomes weak and ownership becomes ambiguous.

### 4. Adopt a conservative collision policy

The planner will classify target paths into explicit action categories such as `create`, `update`, `remove`, `skip`, `warning`, and `danger`.

Expected MVP behavior:
- Missing target + desired managed link -> `create`
- Existing target already matches desired symlink -> `skip`
- Existing target is recorded as managed but no longer matches desired state -> `update` or `remove`
- Existing target is unmanaged and would be overwritten -> `danger`

`apply` will refuse `danger` actions by default and only execute safe actions from the computed plan.

Why this choice:
- Safety is part of the product promise, not a later enhancement.
- Users need a clear answer to "what do you own?" before they trust mutations.

Alternatives considered:
- Best-effort overwrite behavior. Rejected because it makes the tool less trustworthy than manual symlink management.

### 5. Keep the MVP CLI surface intentionally small

The MVP CLI should cover:
- `ssot config validate`
- `ssot profile plan <name>`
- `ssot profile apply <name>`
- `ssot profile doctor <name>`
- `ssot undo`

`plan` and `doctor` will carry most of the explainability burden for now. Dedicated `show`, `explain`, `asset list`, and TUI workflows can follow after the core loop is reliable.

Why this choice:
- It keeps implementation effort focused on the trust loop.
- It avoids shipping multiple read-only commands before the underlying state model is stable.

Alternatives considered:
- Mirror the full draft CLI in the MVP. Rejected because it expands surface area faster than the engine semantics are settled.

## Risks / Trade-offs

- [Symlink-only support may exclude some future hosts] -> Mitigation: keep `mode` in the schema, but validate to `symlink` until more modes are justified.
- [No profile composition means some config duplication] -> Mitigation: accept duplication temporarily and design composition in a separate change once merge semantics are clear.
- [Undo only covers the last successful apply] -> Mitigation: make the limit explicit in CLI output and ensure the journal is written before mutation.
- [State can become stale after manual edits] -> Mitigation: `doctor` compares managed records against the live filesystem and reports stale or broken targets.

## Migration Plan

1. Define a sample config for the current prompt-asset repo.
2. Run `ssot config validate` and `ssot profile plan` against safe test destinations.
3. Apply one profile to a controlled destination and verify recorded state.
4. Intentionally introduce a broken or stale target to validate `doctor`.
5. Run `ssot undo` to confirm last-apply rollback works for recorded symlink changes.

## Open Questions

- Should the next follow-up change prioritize `copy` mode or profile composition?
- Does the MVP need machine-readable plan output immediately, or is human-readable CLI output enough for first validation?
