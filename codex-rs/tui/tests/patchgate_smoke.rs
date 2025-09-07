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
fn patchgate_smoke_repo_apply_and_commit() {
    if !have_git() {
        eprintln!("skipping: git not available");
        return;
    }

    // 1) Init temp repo
    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path();
    assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    // Configure identity locally to allow committing.
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());

    // 2) Build a tiny diff envelope: create README.md with 3 lines.
    let diff_body = r#"diff --git a/README.md b/README.md
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/README.md
@@ -0,0 +1,3 @@
+# Hello
+
+Smoke test
"#;

    let envelope = DiffEnvelope {
        base_ref: "main".to_string(),
        task_id: "TASK-123".to_string(),
        rationale: "seed README".to_string(),
        diff: diff_body.to_string(),
    };

    let mut contract = ChangeContract::default();
    contract.task_id = "TASK-123".to_string();
    contract.allowed_paths = vec!["*.md".to_string()];
    contract.max_files_changed = Some(1);
    contract.max_lines_added = Some(5);
    contract.max_lines_removed = Some(0);
    contract.allow_renames = false;
    contract.allow_deletes = false;
    contract.forbid_binary = true;
    contract.require_tests = false;
    contract.commit_prefix = "chore".to_string();

    // 3) Dry-run gate should pass (no apply/commit)
    let report: ApplyReport = verify_and_apply_patch(
        repo,
        &envelope,
        &contract,
        "Add README stub",
        true,
        WorktreePolicy::InPlace,
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    )
    .expect("dry-run gate");
    assert!(report.checked_ok);
    assert!(!report.applied);
    assert_eq!(report.stats.files_changed, 1);
    assert_eq!(report.stats.lines_added, 3);

    // 4) Apply + commit
    let report2: ApplyReport = verify_and_apply_patch(
        repo,
        &envelope,
        &contract,
        "Add README stub",
        false,
        WorktreePolicy::InPlace,
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    )
    .expect("apply gate");
    assert!(report2.applied && report2.committed);
    assert_eq!(report2.stats.files_changed, 1);
    assert_eq!(report2.stats.lines_added, 3);
    assert!(report2.commit_sha.is_some());

    // 5) Verify commit message contains [TASK_ID]
    let (_ok, msg) = git(repo, ["log", "-1", "--pretty=%B"]);
    assert!(msg.contains("[TASK-123]"), "commit message missing [TASK_ID]: {msg}");
}
