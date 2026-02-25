# codex-git

Helpers for interacting with git, including patch application and worktree
snapshot utilities.

```rust,no_run
use std::path::Path;

use codex_git::{
    apply_git_patch, create_ghost_commit, restore_ghost_commit, ApplyGitRequest,
    CreateGhostCommitOptions,
};

let repo = Path::new("/path/to/repo");

// Apply a patch (omitted here) to the repository.
let request = ApplyGitRequest {
    cwd: repo.to_path_buf(),
    diff: String::from("...diff contents..."),
    revert: false,
    preflight: false,
};
let result = apply_git_patch(&request)?;

// Capture the current working tree as an unreferenced commit.
let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;

// Later, undo back to that state.
restore_ghost_commit(repo, &ghost)?;
```

Pass a custom message with `.message("…")` or force-include ignored files with
`.force_include(["ignored.log".into()])`.
