# Assets-First Rule Expander

Use `scripts/expand_asset_dir.py` when a directory like `Skills/` contains many sibling assets and each child should become one independently togglable flat rule.

The script:

- scans one directory under a real source root
- emits one rule per immediate child
- sorts rules lexicographically
- optionally marks selected entries disabled
- emits a profile snippet that can be pasted under `profiles:`

## Usage

```bash
python scripts/expand_asset_dir.py \
  --root ~/personal-harness-management \
  --under Skills \
  --profile skill-global \
  --to ~/.codex/skills/ \
  --to ~/.config/opencode/skills/ \
  --mode symlink \
  --tag skill \
  --disabled prompt-iteration
```

## Output Shape

```yaml
skill-global:
  rules:
    - select: Skills/brainstorming
      to:
        - ~/.codex/skills/
        - ~/.config/opencode/skills/
      mode: symlink
      tags:
        - skill
    - select: Skills/prompt-iteration
      to:
        - ~/.codex/skills/
        - ~/.config/opencode/skills/
      mode: symlink
      enabled: false
      tags:
        - skill
```

## Important Limits

- It works well for source-assets-first bundles because `--to` is repeatable.
- It emits a profile snippet, not a complete config.
- It only expands immediate children under `--under`.
- It is for authoring convenience only. The final runtime truth remains explicit flat YAML.
- It does not preserve later manual edits if you rerun it and overwrite the same section by hand.
