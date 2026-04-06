## 1. Shared Inspection Layer

- [ ] 1.1 Add a library inspection module with typed views for profile summaries, effective profile detail, and profile explanation data
- [ ] 1.2 Reuse existing config resolution and reconcile logic to populate inspection views without introducing a second planning path

## 2. CLI Commands And Output

- [ ] 2.1 Extend the CLI with `profile list`, `profile show <name>`, and `profile explain <name>` commands
- [ ] 2.2 Add human-readable rendering and `--json` output for the new inspection commands using the shared inspection structs

## 3. Verification And Docs

- [ ] 3.1 Add tests covering deterministic profile listing, effective source root reporting, explanation output, and JSON serialization
- [ ] 3.2 Update `SSOT-manager/README.md` and related docs to describe the new read-only inspection workflow
