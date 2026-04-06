## Context

The current CLI proves the trust loop around `plan`, `apply`, `doctor`, and `undo`, but it still leaves operators blind to basic inspection questions: which profiles exist, what a profile resolves to, and why the current filesystem state produces the plan they see. The draft explicitly treats visibility as first-class and lists `profile list`, `profile show`, and `profile explain` ahead of TUI work.

This change should add a read-only inspection surface without entangling output logic with mutation code. The next thin TUI should consume the same typed inspection data instead of scraping terminal text.

## Goals / Non-Goals

**Goals:**
- Add profile inspection commands that answer discovery and explanation questions without mutating state.
- Introduce a shared inspection layer in the Rust library that can be rendered as human-readable text or JSON.
- Make profile-level source root selection, enabled rules, diagnostics, resolved intents, and current plan state inspectable through stable command output.
- Keep the new commands aligned with the existing deterministic planner and state model.

**Non-Goals:**
- Asset-level browsing or asset search commands in this change.
- Interactive editing, toggling, or confirmation flows.
- Replacing `plan` and `doctor`; those commands remain the mutation-adjacent truth for reconcile and drift.
- Any TUI implementation in this change.

## Decisions

### 1. Add a dedicated inspection module instead of expanding CLI-only formatting

Create a new library-facing inspection layer with typed structs for:
- profile summary rows for `profile list`
- effective profile view for `profile show`
- resolved explanation view for `profile explain`

Why this choice:
- The same inspection model can back CLI text rendering, JSON rendering, and the later TUI.
- It keeps `cli.rs` from becoming the place where config resolution, plan summarization, and presentation all mix together.
- It creates a stable internal contract for tests.

Alternatives considered:
- Add ad hoc printing directly in `cli.rs`. Rejected because the TUI would later need to duplicate or parse those decisions.

### 2. Separate `show` from `explain`

`profile show <name>` should answer "what is this profile?" and return effective configuration data:
- effective source root
- ordered rules
- enabled/disabled state
- destinations, mode, tags, and notes

`profile explain <name>` should answer "what does this profile resolve to right now?" and include:
- diagnostics from profile resolution
- resolved intents
- current plan items and action counts

Why this choice:
- It avoids one overloaded command that mixes static config and live filesystem comparison.
- It matches how operators think: inspect definition first, then explain current outcome.

Alternatives considered:
- A single `show` command with optional flags. Rejected because the output contract becomes harder to understand and test.

### 3. Support JSON output on the new inspection commands

The new commands should default to human-readable output but accept `--json` for machine-readable output. The JSON payloads should map directly to the inspection structs rather than re-serializing formatted terminal strings.

Why this choice:
- It future-proofs the inspection layer for the thin TUI and any scripted usage.
- It avoids baking view formatting into data contracts.

Alternatives considered:
- Human-readable output only. Rejected because the next TUI would then need a parallel internal API change or brittle text parsing.

### 4. Keep plan and doctor as independent commands while reusing their internals

`profile explain` should reuse the existing profile resolution and plan-building path rather than introducing a second planning flow. It can summarize the same `Plan` data that `profile plan` prints today, but it should not replace `plan` or `doctor`.

Why this choice:
- The reconcile engine already contains the authoritative action classification logic.
- Reuse reduces the chance that explanation output diverges from mutation behavior.

Alternatives considered:
- Folding `plan` semantics into `explain` and deprecating `plan`. Rejected because it blurs dry-run mutation preview with general inspection.

## Risks / Trade-offs

- [Inspection structs become too presentation-shaped] -> Mitigation: keep them focused on resolved domain data and summary counts, not terminal layout concerns.
- [JSON output freezes unstable details too early] -> Mitigation: limit the first JSON schema to obvious fields the TUI actually needs and avoid exposing incidental formatting artifacts.
- [The command surface grows before asset-level inspection exists] -> Mitigation: keep this change profile-centered and leave `asset list/status` for a later change.

## Migration Plan

1. Add the library inspection module and its typed views.
2. Extend CLI parsing with `profile list`, `profile show`, and `profile explain`, plus `--json` for those commands.
3. Add tests for text and JSON output shape, effective source root reporting, and explanation content.
4. Update README and draft-adjacent docs to position these commands as the read-only layer the TUI will build on.

## Open Questions

- Should `profile list` include quick summary counts, such as enabled rule count and last-known managed target count, or stay minimal in v1?
- Does `profile explain` need doctor-style managed drift details immediately, or is plan plus resolution detail sufficient for the first thin TUI?
