# codex-git（中文说明）

用于与 git 交互的辅助工具，包括应用补丁以及工作树快照等功能。

```rust,no_run
use std::path::Path;

use codex_git::{
    apply_git_patch, create_ghost_commit, restore_ghost_commit, ApplyGitRequest,
    CreateGhostCommitOptions,
};

let repo = Path::new(\"/path/to/repo\");

// 将补丁（此处省略内容）应用到仓库。
let request = ApplyGitRequest {
    cwd: repo.to_path_buf(),
    diff: String::from(\"...diff contents...\"),
    revert: false,
    preflight: false,
};
let result = apply_git_patch(&request)?;

// 把当前工作树保存成一个“幽灵提交”（不挂在任何引用上）。
let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;

// 之后可以将仓库状态恢复到该幽灵提交。
restore_ghost_commit(repo, &ghost)?;
```

你可以通过 `.message(\"…\")` 自定义提交信息，或者通过 `.force_include([\"ignored.log\".into()])` 强制包含原本会被忽略的文件。

