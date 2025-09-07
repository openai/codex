use std::path::Path;
use std::process::Command;

use codex_tui::git_guard::{verify_and_apply_patch, DiffEnvelope, WorktreePolicy};
use codex_tui::change_contract::ChangeContract;

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

fn git<I, S>(repo: &Path, args: I) -> (bool, String)
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let out = Command::new("git").arg("-C").arg(repo).args(args).output();
    match out {
        Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stdout).to_string()),
        Err(_) => (false, String::new()),
    }
}

#[test]
fn post_apply_ci_failure_triggers_rollback_no_commit_and_cleanup() {
    if !have_git() {
        eprintln!("skipping: git not available");
        return;
    }

    // Init repo with an initial commit
    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path();
    assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());
    std::fs::write(repo.join("base.txt"), b"base\n").unwrap();
    assert!(Command::new("git").args(["add", "-A"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["commit", "-m", "chore: base"]).current_dir(repo).status().unwrap().success());
    let (_ok, base_sha) = git(repo, ["rev-parse", "--short", "HEAD"]);
    let base_sha = base_sha.trim().to_string();

    // A diff to add a file, but CI will fail after apply
    let diff_body = r#"diff --git a/new.txt b/new.txt
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,1 @@
+fail_me
"#;

    let envelope = DiffEnvelope {
        base_ref: base_sha.clone(),
        task_id: "TASK-CI-ROLL".to_string(),
        rationale: "simulate failing CI".to_string(),
        diff: diff_body.to_string(),
    };

    let mut contract = ChangeContract::default();
    contract.task_id = envelope.task_id.clone();
    contract.allowed_paths = vec!["**/*.txt".to_string(), "new.txt".to_string()];
    contract.max_files_changed = Some(2);
    contract.max_lines_added = Some(10);
    contract.max_lines_removed = Some(0);
    contract.allow_renames = false;
    contract.allow_deletes = false;
    contract.forbid_binary = true;
    contract.require_tests = false; // we'll fail in post-apply manually
    contract.commit_prefix = "chore".to_string();

    // CI closure: always fail
    fn failing_ci(_: &Path) -> color_eyre::eyre::Result<()> {
        Err(color_eyre::eyre::eyre!("simulated CI failure"))
    }

    // Apply expecting an error; ensure no commit and cleanup done
    let res = verify_and_apply_patch(
        repo,
        &envelope,
        &contract,
        "Add file",
        false,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: envelope.base_ref.clone(), task_id: envelope.task_id.clone() },
        Some(failing_ci as fn(&Path) -> color_eyre::eyre::Result<()>),
    );

    assert!(res.is_err(), "expected CI to fail and gate to error");

    // Ensure branch hasn't advanced
    let (_ok, auto_sha) = git(repo, ["rev-parse", "--short", "autopilot/TASK-CI-ROLL"]);
    let auto_sha = auto_sha.trim();
    assert_eq!(auto_sha, base_sha, "branch should not advance on CI failure");

    // Worktree path should be removed
    let ep = repo.join(".worktrees").join("autopilot").join(&envelope.task_id);
    assert!(!ep.exists(), "ephemeral worktree should be removed after failure");
}
