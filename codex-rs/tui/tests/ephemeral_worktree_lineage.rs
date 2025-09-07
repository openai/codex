use std::path::Path;
use std::process::Command;

use codex_tui::git_guard::{verify_and_apply_patch, ApplyReport, DiffEnvelope, WorktreePolicy};
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
fn ephemeral_lineage_and_cleanup_check_only_and_apply() {
    if !have_git() {
        eprintln!("skipping: git not available");
        return;
    }

    // Init repo with an initial commit on main
    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path();
    assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());

    // make an initial commit to have a valid base_ref
    std::fs::write(repo.join(".gitignore"), b"/target\n").unwrap();
    assert!(Command::new("git").args(["add", "-A"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["commit", "-m", "chore: init"]).current_dir(repo).status().unwrap().success());
    let (_ok, base_sha) = git(repo, ["rev-parse", "--short", "HEAD"]);
    let base_sha = base_sha.trim().to_string();

    // Build a diff to add README.md
    let diff_body = r#"diff --git a/README.md b/README.md
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/README.md
@@ -0,0 +1,2 @@
+Hello
+World
"#;

    let envelope = DiffEnvelope {
        base_ref: base_sha.clone(),
        task_id: "TASK-E1".to_string(),
        rationale: "verify ephemeral lineage + cleanup".to_string(),
        diff: diff_body.to_string(),
    };

    let mut contract = ChangeContract::default();
    contract.task_id = envelope.task_id.clone();
    contract.allowed_paths = vec!["**/*.md".to_string(), "README.md".to_string()];
    contract.max_files_changed = Some(2);
    contract.max_lines_added = Some(10);
    contract.max_lines_removed = Some(0);
    contract.allow_renames = false;
    contract.allow_deletes = false;
    contract.forbid_binary = true;
    contract.require_tests = false;
    contract.commit_prefix = "chore".to_string();

    // Dry-run (ephemeral) should not leave the worktree behind
    let report: ApplyReport = verify_and_apply_patch(
        repo,
        &envelope,
        &contract,
        "Add README",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: envelope.base_ref.clone(), task_id: envelope.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    )
    .expect("dry-run ok");
    assert!(report.checked_ok);
    let ep = repo.join(".worktrees").join("autopilot").join(&envelope.task_id);
    assert!(
        !ep.exists(),
        "ephemeral worktree should be removed after check_only"
    );

    // Apply (ephemeral) should create a commit on autopilot branch and remove worktree
    // Use a different task_id to avoid branch reuse from the dry-run
    let envelope2 = DiffEnvelope {
        base_ref: base_sha.clone(),
        task_id: "TASK-E2".to_string(),
        rationale: envelope.rationale.clone(),
        diff: diff_body.to_string(),
    };
    let mut contract2 = ChangeContract::default();
    contract2.task_id = envelope2.task_id.clone();
    contract2.allowed_paths = contract.allowed_paths.clone();
    contract2.max_files_changed = contract.max_files_changed;
    contract2.max_lines_added = contract.max_lines_added;
    contract2.max_lines_removed = contract.max_lines_removed;
    contract2.commit_prefix = contract.commit_prefix.clone();
    let report2: Result<ApplyReport, _> = verify_and_apply_patch(
        repo,
        &envelope2,
        &contract2,
        "Add README",
        false,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: envelope2.base_ref.clone(), task_id: envelope2.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    );
    let report2 = report2.expect("apply ok");
    assert!(report2.applied && report2.committed);
    assert!(report2.commit_sha.is_some());
    assert!(
        !ep.exists(),
        "ephemeral worktree should be removed after apply"
    );

    // The autopilot branch should exist and differ from base
    let (_ok, auto_sha) = git(repo, ["rev-parse", "--short", "autopilot/TASK-E2"]);
    let auto_sha = auto_sha.trim();
    assert!(!auto_sha.is_empty());
    assert_ne!(auto_sha, base_sha);
}
