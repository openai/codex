# codex-git-utils

Helpers for interacting with git, including patch application. The crate also
exposes a lightweight baseline API for internal directories that use git only
as a resettable diff mechanism: `ensure_git_baseline_repository` preserves a
usable `root/.git` baseline or creates one when it is missing or unusable,
`reset_git_repository` replaces `root/.git` with a fresh one-commit baseline,
and `diff_since_latest_init` returns structured file changes plus a unified
diff from that baseline to the current directory contents.

```rust,no_run
use std::path::Path;

use codex_git_utils::{apply_git_patch, ApplyGitRequest};

let repo = Path::new("/path/to/repo");

// Apply a patch (omitted here) to the repository.
let request = ApplyGitRequest {
    cwd: repo.to_path_buf(),
    diff: String::from("...diff contents..."),
    revert: false,
    preflight: false,
};
let result = apply_git_patch(&request)?;
```
