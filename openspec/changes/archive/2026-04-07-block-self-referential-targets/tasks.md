## 1. Planner Guard

- [x] 1.1 Add a path-resolution helper that computes the effective target path after resolving existing ancestor symlinks
- [x] 1.2 Block plan items whose effective target path overlaps the desired source path or subtree, and mark them as non-forceable danger with an explicit reason

## 2. Regression Coverage

- [x] 2.1 Add a flow fixture/profile that reproduces a parent-directory symlink pointing back into the source tree
- [x] 2.2 Add regression tests asserting both `profile plan` and `profile apply` refuse the self-referential overlap
- [x] 2.3 Verify ordinary matching leaf symlinks still classify as `skip` when no ancestor self-reference exists
