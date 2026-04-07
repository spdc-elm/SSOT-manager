## Context

The current planner is intentionally conservative: any unmanaged collision becomes `danger`, and apply refuses the whole plan. That makes the engine trustworthy, but it is also the main blocker when the operator wants the manager to adopt an existing file or directory that was created manually before SSOT-manager was introduced.

The obvious naive answer would be "just let copy mode overwrite after keeping a copy somewhere", but that mixes materialization mode with operator intent. `copy`, `symlink`, and `hardlink` describe how the desired target should exist after apply; they do not answer whether it is safe to replace an unmanaged target in the first place. The backup decision is a separate force path that must remain explicit in plan, apply, journal, undo, and UI semantics.

This change is cross-cutting because it alters danger handling in reconcile, introduces backup persistence for previously unmanaged targets, changes undo guarantees, and adds explicit operator confirmation semantics to the TUI and CLI.

## Goals / Non-Goals

**Goals:**
- Preserve `danger` as the default planner outcome for unmanaged collisions.
- Add an explicit force-with-backup apply path that can take over eligible unmanaged targets only after recording a restorable backup.
- Ensure undo can restore overwritten unmanaged files, directories, and symlinks from manager-controlled backup data.
- Add operator-facing flows that make the force action explicit in both CLI and TUI.
- Make the behavior easy to test in sandbox destinations before using it on real global config paths.

**Non-Goals:**
- Making ordinary `profile apply` silently overwrite unmanaged content.
- Treating `copy` mode itself as an implicit backup or force mechanism.
- Supporting force takeover for every possible filesystem node type; regular files, directories, and symlinks are the target scope.
- Introducing an always-on auto-backup layer for every safe apply, including already managed updates.

## Decisions

### 1. Keep planner classification conservative, but mark a subset of dangers as forceable

Unmanaged collisions should still plan as `danger`. The difference is that the planner and inspection layers should distinguish between:
- non-forceable dangers
- forceable unmanaged collisions that can be replaced if the operator chooses backup overwrite

Why this choice:
- It preserves the existing trust model: danger remains visible and blocking by default.
- It avoids collapsing "this is risky" into "this is safe now because backup exists".
- It gives the UI and CLI enough structure to present an explicit operator choice.

Alternatives considered:
- Reclassify forceable unmanaged collisions as `warning`. Rejected because it would weaken the visual severity of replacing unmanaged content.

### 2. Model backup overwrite as an explicit apply mode, not a property of materialization mode

The implementation should add an explicit apply path such as `profile apply --force-with-backup`, and TUI apply should require a second confirmation action when the current plan contains only forceable dangers.

Why this choice:
- It keeps operator intent explicit.
- It avoids overloading `copy` semantics with ownership takeover behavior.
- It keeps future extension possible across `symlink`, `copy`, and `hardlink`.

Alternatives considered:
- Add a per-rule flag like `takeover: true`. Rejected because overwrite intent should be an operator decision at apply time, not a permanently baked-in config default.

### 3. Persist overwritten unmanaged targets as restorable backup snapshots under state storage

Before force apply mutates an unmanaged target, the manager should capture a restorable backup in state-owned storage, record its metadata in the journal, and then perform the desired materialization. Undo should use the recorded backup artifact rather than trying to reconstruct unmanaged content from path snapshots alone.

Why this choice:
- Existing `PathState` metadata is enough to verify state, but not enough to recreate arbitrary unmanaged file or directory contents.
- Using state-owned backup artifacts makes undo practical for real takeover flows.

Alternatives considered:
- Store only hashes or tree metadata. Rejected because that cannot restore content.
- Reuse the target path in place as the "backup". Rejected because it would not survive the overwrite.

### 4. Keep undo strict: only restore backups recorded by the last successful force apply

Undo should continue to operate only on the last successful journal. When that journal contains a backup-overwrite entry, undo should restore the backup if the current post-apply target still matches the recorded post-apply state.

Why this choice:
- It preserves the existing "last successful apply only" mental model.
- It keeps backup restore behavior aligned with current undo safety checks.

Alternatives considered:
- Add a general backup history browser immediately. Rejected because it expands scope beyond the immediate migration need.

### 5. Make the first TUI force flow explicit and lightweight

The TUI should not add a complex modal system. Instead, if the selected profile plan contains only forceable dangers, the first `a` should surface a backup-overwrite confirmation state and the second `a` should execute the force apply. Any non-forceable danger should continue to block.

Why this choice:
- It matches the user's desired "press a again to force" interaction.
- It keeps the TUI profile-centered and low-complexity.
- It still makes the force step explicit and reversible.

Alternatives considered:
- Force immediately on first `a`. Rejected because it is too easy to trigger accidentally.
- Add a new dedicated force key only. Rejected because a repeated apply confirmation is simpler for the first version.

## Risks / Trade-offs

- [Backups can consume significant space for large directories] -> Mitigation: scope the first version to explicit force applies only and record clear backup ownership in the journal.
- [Operator may misunderstand forceable vs non-forceable danger] -> Mitigation: surface the distinction clearly in plan output, CLI errors, and TUI status text.
- [Backup restore increases undo complexity] -> Mitigation: keep backup restoration confined to the existing last-apply journal model instead of introducing general historical restore.
- [TUI repeated-apply confirmation can be confusing] -> Mitigation: show a strong status message that the next apply will force overwrite with backup, and reset that state when selection or plan context changes.

## Migration Plan

1. Extend OpenSpec requirements for forceable danger semantics, backup persistence, and TUI force confirmation.
2. Add backup snapshot storage and journal metadata for overwritten unmanaged targets.
3. Add an explicit force-with-backup apply path in the CLI and reconcile library.
4. Extend the TUI apply flow with repeated-confirmation handling for forceable dangers.
5. Add integration coverage using sandbox copy/symlink targets and document recommended migration workflows.

## Open Questions

- Should force-with-backup be allowed only when all dangers in the plan are forceable, or should it skip non-forceable dangers while still applying forceable ones?
- Should the CLI force path require an additional interactive confirmation when running on a TTY, or is the explicit flag sufficient for the first version?
