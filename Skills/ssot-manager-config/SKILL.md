---
name: ssot-manager-config
description: Draft, revise, validate, and explain ssot-manager config files. Use when user needs to translate an asset repo into flat profile/rule YAML, choose between assets-first and project-first config patterns, add prompt compositions or profile source roots, or generate one-rule-per-asset configs for skills and similar directories.
---

# SSOT Manager Config

Draft runtime config for `ssot-manager` without inventing semantics.

## Workflow

1. Read the current config or asset layout before proposing changes.
2. Treat `profile` as an applyable bundle and `rule` as the per-asset sync toggle.
3. Prefer flat YAML as the source of truth. Do not assume profile inheritance, profile composition, or implicit overlay semantics.
4. Prefer assets-first authoring unless the user explicitly wants project-first.
5. When "assets-first" is ambiguous, default to source-assets-first: one applyable profile for the bundle, with one rule per source asset and as many `to` destinations on that rule as the bundle needs.
6. When per-asset enable/disable matters, emit one rule per asset. Do not keep a single wildcard rule and pretend it supports per-item toggles.
7. Validate with `ssot-manager --config <path> config validate` when the binary is installed, or `cargo run -- --config <path> config validate` when working from the repo checkout.

## Key Rules

- Interpret `select` relative to the profile's effective `source_root`, not as an absolute filesystem path.
- Interpret relative `to` destinations relative to the config file directory.
- Use profile-level `source_root` only when one bundle genuinely resolves assets from a different root.
- Do not equate assets-first with "one profile per installation surface". That is only one possible authoring pattern.
- When the same source asset should sync to multiple consumers as one bundle, prefer one rule with multiple `to` destinations inside one profile.
- The clap command name shown in help may be `ssot`, but the installed binary and package name are currently `ssot-manager`. Do not assume `ssot` exists on PATH.
- Keep rule order deterministic. For generated per-asset rules, sort by asset basename.
- Use `enabled: false` only when a rule is intentionally disabled. Omit it for enabled rules.
- Avoid overlapping profiles that manage the same target paths unless the user explicitly wants separate, conflicting bundles. The runtime treats that as cross-profile ownership, not inheritance.
- In examples, pick one destination family such as `~/.codex/...` or `~/.agents/...` unless the user explicitly wants both. Do not duplicate them casually.

## References

- Read `references/config-schema.md` for the runtime model, path resolution rules, and supported fields.
- Read `references/patterns.md` for assets-first and project-first authoring patterns plus anti-patterns.
- Read `references/assets-first-expander.md` before using the helper script.

## Script

Use `scripts/expand_asset_dir.py` when the user already knows they want flat YAML but has many sibling assets such as `Skills/<name>` that should become one rule each.

- The script scans one asset directory and emits a flat profile snippet with one rule per child.
- The script does not change ssot-manager runtime semantics.
- Use it to reduce authoring work, not to hide config logic from the user.

## Output Standard

When writing config for the user:

- Return a complete YAML config unless the user asked for a partial patch.
- Keep comments minimal.
- Make the mapping from asset layout to rules obvious on inspection.
- If you use the helper script, show the invocation and the generated flat output separately.
