# SSOT Manager Config Schema

Use this file as the authoritative runtime model when drafting config.

## Mental Model

- `profile`: an applyable bundle the operator can `plan`, `apply`, `doctor`, and `undo`
- `rule`: a per-asset sync rule inside a profile
- `asset`: a file or directory matched by a rule's `select`

There is no profile inheritance, include, or merge semantics in the current runtime.

## Runtime Shape

```yaml
version: 1
source_root: /path/to/asset-repo

compositions:
  codex-agent:
    output: build/prompts/codex/AGENTS.generated.md
    variables:
      host: codex
    inputs:
      - path: Agents/assistant.md
        wrapper:
          before: "<assistant path=\"{{path}}\">\n"
          after: "\n</assistant>\n"
    renderer:
      kind: concat
      outer_wrapper:
        before: "<prompt host=\"{{host}}\">\n"
        after: "\n</prompt>\n"

profiles:
  skill-global:
    rules:
      - select: Skills/alpha
        to:
          - ~/.codex/skills/
          - ~/.config/opencode/skills/
        mode: symlink
      - select: Skills/beta
        to:
          - ~/.codex/skills/
          - ~/.config/opencode/skills/
        mode: copy
        ignore:
          - "**/.DS_Store"
          - "**/Thumbs.db"
        enabled: false
        tags:
          - skill
        note: disabled by default
```

## Path Rules

- Top-level `source_root` may be absolute or relative.
- Relative `source_root` resolves relative to the config file directory.
- `profiles.<name>.source_root` follows the same rule and overrides the top-level root for that profile only.
- `select` is matched relative to the profile's effective `source_root`.
- Relative `to` destinations resolve relative to the config file directory.
- `~/` expands from `HOME`.
- If `to` ends with `/`, already exists as a directory, or one rule matches multiple assets, the runtime appends each source basename to the destination. Example: `select: docs` to `.../sys1/` becomes `.../sys1/docs`. To sync the contents directly into `.../sys1/`, set `source_root` to `.../docs` and sync `select: "*"`.

## Supported Fields

### Top Level

- `version`: must be `1`
- `source_root`: required
- `compositions`: optional map of prompt compositions
- `profiles`: required map of named profiles

### Profile

- `source_root`: optional override for that profile
- `requires`: optional list of composition names
- `rules`: ordered list of sync rules

### Rule

- `select`: required glob or relative asset path
- `to`: required ordered list of destinations
- `mode`: required, one of `symlink`, `copy`, `hardlink`
- `ignore`: optional ordered list of glob patterns matched relative to the selected asset root
- `enabled`: optional, defaults to `true`
- `tags`: optional list of strings
- `note`: optional string

## Important Constraints

- Rule toggles are per rule, not per wildcard expansion result.
- If per-skill toggles matter, generate one rule per skill directory.
- `ignore` is explicit operator policy, not a hidden runtime default. Use it mainly on `copy` and `hardlink` directory rules when hosts emit metadata files inside otherwise healthy managed trees.
- `ignore` changes directory comparison and materialization semantics. Ignored descendants are omitted from desired tree evaluation, post-apply verification, doctor drift checks, and undo post-state safety checks.
- Profiles that target the same paths do not layer. The runtime will report targets managed by another profile.
- `requires` refers to prompt compositions only, not other profiles.

## Validation Commands

Use these when the config is concrete:

```bash
ssot-manager --config ssot.yaml config validate
ssot-manager --config ssot.yaml profile list
ssot-manager --config ssot.yaml profile show skill-global
ssot-manager --config ssot.yaml profile explain skill-global

# from the repo checkout
cargo run -- --config ssot.yaml config validate
```
