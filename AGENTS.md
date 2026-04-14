# Collaboration Notes

- When synchronizing long-lived branches such as `main` and `dev`, prefer `merge` over bilateral `cherry-pick`. Use `cherry-pick` only for narrow exceptional backports, because mirrored picks make the history diverge and keep pull requests artificially mergeable.
- For Rust changes, run `cargo fmt --check` before commit. If it fails, fix formatting with `cargo fmt` and re-run the check before pushing.
