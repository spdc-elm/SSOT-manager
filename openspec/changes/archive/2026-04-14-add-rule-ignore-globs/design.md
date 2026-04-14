## Context

`copy` and `hardlink` directory rules currently rely on exact recursive tree equality. That keeps the model simple, but it means host-generated junk files such as `.DS_Store` are treated as real managed drift and can also interfere with doctor and undo safety checks.

This change is cross-cutting because ignore policy affects config parsing, resolved intent data, directory reconciliation, post-apply verification, doctor comparisons, and undo safety checks. The design needs to preserve the current strict default while letting operators opt into platform-specific exceptions deliberately.

## Goals / Non-Goals

**Goals:**
- Add an explicit rule-level `ignore` field with predictable glob semantics.
- Keep default behavior unchanged when `ignore` is absent.
- Make `plan`, `apply`, `doctor`, and `undo` agree on which directory descendants count toward managed equivalence.
- Document practical platform guidance for `copy` and `hardlink` directory rules without hard-coding host-specific junk into the runtime.

**Non-Goals:**
- No built-in macOS, Windows, or Linux ignore defaults.
- No regex-based matcher language or global ignore file format in this change.
- No attempt to preserve ignored target-only files during a directory rewrite caused by other non-ignored changes.
- No change to `symlink` behavior for single-link targets; `ignore` only matters when comparing or materializing directory trees.

## Decisions

### 1. `rule.ignore` is an optional ordered list of glob patterns

Each rule may declare `ignore: [<glob>, ...]`. Patterns are interpreted relative to the matched asset root, not as absolute filesystem paths and not relative to the config file. `glob` is the right fit here because the rest of the config already teaches glob semantics through `select`, while regex would make simple path exclusions harder to author and review.

Alternatives considered:
- Hard-coded platform junk ignores: rejected because it hides policy inside the runtime and makes strict mirroring impossible to reason about.
- Regex patterns: rejected because they are more error-prone for operators and inconsistent with the rest of the config surface.
- Profile-level ignore only: rejected for the first iteration because different rules in the same profile may need different tolerance.

### 2. Ignore patterns define the desired directory tree, not just doctor output filtering

For directory assets, the runtime should exclude ignore-matched descendants from equivalence checks and from `copy`/`hardlink` materialization. That keeps `skip`, post-apply verification, doctor drift, and undo safety aligned around one notion of "managed content."

Alternatives considered:
- Apply ignore only in doctor: rejected because plan and doctor would disagree.
- Apply ignore only to target-only extras: rejected because source-side ignored junk would still be materialized and reintroduced on updates.

### 3. Undo uses the ignore policy captured at apply time

Undo must compare the current target against the last applied post-state using the ignore semantics that were in force when that journal was recorded. This keeps undo deterministic even if the operator later edits the config. The journal therefore needs enough comparison metadata to re-run post-apply safety checks with the recorded ignore policy.

Alternatives considered:
- Read the current config during undo: rejected because `undo` currently operates from state/journal data and should remain able to revert the last apply without requiring the current config to still exist or match.

### 4. Documentation carries host-specific recommendations instead of runtime defaults

The `ssot-manager-config` skill docs and schema/reference docs should show practical examples such as `**/.DS_Store`, `**/._*`, `**/Thumbs.db`, or `**/desktop.ini` as operator choices for `copy`/`hardlink` directory rules. The runtime itself remains neutral.

Alternatives considered:
- Ship built-in presets only: rejected for this change because presets still introduce hidden policy unless explicitly declared, and they are not necessary to unlock the core capability.

## Risks / Trade-offs

- [Pattern semantics are misunderstood] → Mitigation: document clearly that patterns are matched against relative descendant paths under the matched asset root and add focused tests for nested paths.
- [Ignore masks a real managed file unexpectedly] → Mitigation: keep the feature opt-in per rule, preserve strict defaults, and document it primarily for platform-generated metadata.
- [Journal/state logic becomes more complex] → Mitigation: keep the ignore policy narrow and reuse the same matcher logic across plan, verify, doctor, and undo rather than duplicating ad hoc comparisons.
- [Operators expect ignored target-only junk to survive every update] → Mitigation: document that ignore changes comparison semantics, not directory merge semantics during rewrites.

## Migration Plan

This is an additive config change. Existing configs continue to validate and behave exactly as before.

Recommended rollout:
1. Add `ignore` to the relevant `copy`/`hardlink` directory rules.
2. Validate the config and inspect the profile plan/doctor output.
3. Re-apply the profile when the operator wants the new ignore policy and materialized tree semantics to become the new recorded baseline.

Rollback is just removing the `ignore` field and re-applying the profile.

## Open Questions

- None for this proposal. The key product decision is explicit: no built-in default ignores, only configured rule-level glob patterns plus documentation guidance.
