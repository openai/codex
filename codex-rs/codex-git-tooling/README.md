# codex-git-tooling

Utilities for working with git state in Codex. The crate currently provides
helpers to capture a lightweight snapshot of the workspace as a "ghost commit"
without mutating the user's repository, and later restore the checkout to that
state.

## Ghost commits

There are two top-level functions:

* `create_ghost_commit` – walks the working tree, writes the contents into a
  temporary index, and produces a commit (not referenced by any branch) that
  represents the snapshot. The result is a `GhostCommit` carrying the commit ID
  and the parent HEAD if one was present.
* `restore_ghost_commit` – checks out the tree at a ghost commit into the
  current working directory, removing files that are absent and restoring
  tracked and forced-included files.

### Creating snapshots

```rust,no_run
use std::path::Path;

use codex_git_tooling::{create_ghost_commit, CreateGhostCommitOptions};

let repo = Path::new("/path/to/repo");

let ghost = create_ghost_commit(
    &CreateGhostCommitOptions::new(repo)
        // Optional custom message; defaults to "codex snapshot".
        .message("codex ghost snapshot")
        // Force-include paths that would otherwise be ignored.
        .force_include(vec!["ignored.log".into()]),
)?;

println!("ghost commit id: {}", ghost.id());
```

`create_ghost_commit` returns an error if the directory is not a git repository,
or if any of the forced include paths are not relative or attempt to escape the
repository root.

### Restoring snapshots

```rust,no_run
use std::path::Path;

use codex_git_tooling::{restore_ghost_commit, GhostCommit};

let repo = Path::new("/path/to/repo");
let ghost = GhostCommit::new("cafebabe".to_string(), None);

restore_ghost_commit(repo, &ghost)?;
```

Restoring wipes untracked files in the checkout (ignoring `.git`) before
copying the snapshot contents back in, then recreates tracked files and
symlinks so that the working tree mirrors the ghost commit.

## Error handling

All functions return `Result<_, GitToolingError>`. The error enum reports
command failures, non-UTF-8 output, non-git directories, invalid force-include
paths, IO issues, or failures while walking the filesystem. This allows callers
to surface precise information to end users.

## Tests

Unit tests under `src/lib.rs` exercise snapshot/restore behaviour with
temporary repositories, including repositories without commits, custom commit
messages, force-included ignored files, and error paths for invalid inputs.
