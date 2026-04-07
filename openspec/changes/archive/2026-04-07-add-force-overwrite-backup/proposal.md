## Why

The current SSOT manager treats every unmanaged collision as a hard stop. That is the right default, but it creates friction for real adoption because operators often need to take over an existing config file intentionally, especially when migrating from manually managed prompt files to SSOT-managed targets.

We need a controlled escape hatch now: the operator should be able to force specific danger actions only when the manager first records a restorable backup of the overwritten unmanaged target. That keeps the default safety model intact while making migration and first-time adoption practical.

## What Changes

- Add an explicit force-with-backup apply path for unmanaged collisions that would otherwise be classified as `danger`.
- Persist restorable backups of overwritten unmanaged files, directories, or symlinks in manager-controlled state so `undo` can restore the previous content.
- Keep default `profile apply` behavior conservative: danger still blocks unless the operator explicitly chooses the backup-overwrite path.
- Add a CLI force option for backup overwrite and a TUI confirmation flow where pressing apply again confirms the force-with-backup action for forceable dangers.
- Make the initial implementation easy to test in sandbox profiles such as the copy-based OpenCode example rather than only through real home-directory targets.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `sync-reconciliation`: Extend plan/apply behavior so unmanaged collisions remain `danger` by default, but eligible dangers can be forced with explicit backup-aware apply semantics.
- `managed-state-and-doctor`: Extend journal and undo behavior so overwritten unmanaged targets can be backed up before force apply and restored during undo.
- `terminal-ui-shell`: Extend the TUI apply flow so forceable dangers can be confirmed explicitly from the UI instead of requiring a separate out-of-band command.

## Impact

- Changes reconcile planning and apply execution in `SSOT-manager/src/reconcile.rs`.
- Extends state/journal persistence and undo restore logic in `SSOT-manager/src/state.rs`.
- Expands CLI and TUI action semantics for explicit force-with-backup operations.
- Adds integration coverage for unmanaged collision backup, forced apply, and undo restoration.
