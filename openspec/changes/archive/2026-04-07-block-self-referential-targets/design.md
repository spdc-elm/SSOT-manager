## Context

The SSOT manager already blocks ordinary unmanaged collisions and verifies post-apply state, but that safety model assumes the concrete target path is an honest representation of where mutations will land. A symlinked ancestor directory breaks that assumption: `target/foo.md` can appear separate from `source/foo.md` textually while actually resolving back into the source tree at mutation time.

This is especially dangerous for symlink mode because `remove_existing_path(target)` can end up deleting or replacing the source-side asset that the system is also trying to manage. Backup and undo can reduce damage after the fact, but that is the wrong control plane for this topology; the planner should reject it before mutation.

## Goals / Non-Goals

**Goals:**
- Detect when a target's effective materialization path overlaps the desired source path or subtree.
- Treat that overlap as a non-forceable danger during planning so CLI and TUI inherit the same protection automatically.
- Add regression coverage for the rare but high-risk "symlinked parent points back into source" case.

**Non-Goals:**
- Reworking the broader danger/warning taxonomy.
- Changing the semantics of legitimate single-file symlink targets that already match the desired source.
- Solving every conceivable symlink-cycle or filesystem aliasing problem in one change.

## Decisions

### 1. Detect overlap using the effective target path after resolving existing ancestors

The reconciler will compute an effective materialization path for each target by resolving symlinked existing ancestors of the target path while preserving the intended final leaf name. It will then compare that effective path against the desired source path and treat any overlap as self-referential.

Why this choice:
- It catches the dangerous case that actually matters: where mutations land after ancestor symlinks are followed.
- It does not misclassify ordinary leaf symlinks that already match the desired source and should remain `skip`.
- It is implementation-local and does not require a larger state model rewrite.

Alternatives considered:
- Only compare the raw target string against the source path. Rejected because ancestor symlinks make the raw path unreliable.
- Rely on post-apply verification and undo instead of planning-time checks. Rejected because the mutation can already hit the source tree before those safeguards activate.

### 2. Treat self-referential overlap as non-forceable danger

If the effective target path equals the source path, sits inside the source subtree, or is an ancestor of the source path, the planner will emit a `danger` action with `forceable: false`.

Why this choice:
- A forced backup-overwrite path is appropriate for replaceable unmanaged content, not for source/target aliasing mistakes.
- Preventing mutation entirely is more defensible than attempting to recover from a topology that undermines the model's source/target separation.

Alternatives considered:
- Classify these cases as warnings. Rejected because warnings are still applied.
- Allow `--force-with-backup` to proceed. Rejected because "backup then overwrite" is not a safe abstraction when the target resolves inside the source tree.

## Risks / Trade-offs

- [Overlap detection could reject unusual but legitimate path setups] -> Mitigation: only block actual path overlap after ancestor resolution, not every symlinked parent.
- [Platform-specific path resolution could introduce edge cases] -> Mitigation: keep the helper narrow, based on existing ancestor resolution and normalized path comparisons, and cover the regression case in integration tests.
- [This may expose previously hidden user config mistakes] -> Mitigation: return an explicit danger reason that explains the source/target overlap instead of a generic collision message.

## Migration Plan

1. Add the `sync-reconciliation` spec delta for self-referential target blocking.
2. Implement effective-target overlap detection in the planner.
3. Add regression tests for parent-directory symlink aliasing and verify ordinary matching file symlinks still plan as `skip`.

## Open Questions

- None. The product choice is to block self-referential overlaps before mutation rather than trying to recover from them afterward.
