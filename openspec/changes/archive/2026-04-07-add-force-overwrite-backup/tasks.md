## 1. Planner And CLI Force Path

- [x] 1.1 Extend reconcile planning data so unmanaged collisions remain `danger` but can also be marked as forceable when backup overwrite is supported
- [x] 1.2 Add an explicit CLI apply path for force-with-backup behavior without changing ordinary `profile apply` defaults
- [x] 1.3 Ensure ordinary apply still blocks all danger actions while the new force path only proceeds when every danger in the plan is forceable

## 2. Backup Persistence And Undo

- [x] 2.1 Add state-owned backup storage for overwritten unmanaged files, directories, and symlinks
- [x] 2.2 Extend the apply journal schema to record backup metadata for force-overwrite entries
- [x] 2.3 Update undo so last-apply restoration can recover overwritten unmanaged content from recorded backups

## 3. TUI Confirmation Flow

- [x] 3.1 Extend TUI plan/detail state so forceable dangers are distinguishable from non-forceable dangers
- [x] 3.2 Add a repeated-apply confirmation flow where the first apply arms force-with-backup and the second apply executes it
- [x] 3.3 Reset pending force confirmation when profile selection or plan context changes, and surface clear status text for the backup-overwrite action

## 4. Verification And Documentation

- [x] 4.1 Add integration coverage for force-with-backup replacing unmanaged files, directories, and symlinks and restoring them via undo
- [x] 4.2 Add sandbox-oriented tests around copy/symlink takeover flows so the feature can be validated without touching real home-directory config
- [x] 4.3 Update README and operator guidance to explain ordinary danger blocking versus explicit backup-overwrite force apply
