use std::path::Path;
use std::process::Command;

use codex_tui::git_guard::{verify_and_apply_patch, DiffEnvelope, WorktreePolicy};
use codex_tui::change_contract::ChangeContract;

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

#[test]
fn blocks_path_traversal_and_git_internals() {
    if !have_git() {
        eprintln!("skipping: git not available");
        return;
    }

    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path();
    assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["commit", "--allow-empty", "-m", "chore: base"]).current_dir(repo).status().unwrap().success());

    // 1) Traversal attempt
    let diff_trav = r#"diff --git a/../../evil.txt b/../../evil.txt
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/../../evil.txt
@@ -0,0 +1 @@
+oops
"#;
    let env1 = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-TRAV".into(), rationale: "trav".into(), diff: diff_trav.into() };
    let mut contract = ChangeContract::default();
    contract.task_id = env1.task_id.clone();
    // empty allowed_paths => allow-all, but traversal must be rejected by safety rule

    let rep1 = verify_and_apply_patch(
        repo,
        &env1,
        &contract,
        "trav",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env1.base_ref.clone(), task_id: env1.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("rep1");
    assert!(!rep1.checked_ok);
    assert!(rep1.contract_violations.iter().any(|v| v.contains("traversal")));

    // 2) Touch .git/ path
    let diff_git = r#"diff --git a/.git/config b/.git/config
index e69de29..4b825dc 100644
--- a/.git/config
+++ b/.git/config
@@ -0,0 +1,1 @@
+oops
"#;
    let env2 = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-GIT".into(), rationale: "git".into(), diff: diff_git.into() };
    let mut contract2 = ChangeContract::default();
    contract2.task_id = env2.task_id.clone();

    let rep2 = verify_and_apply_patch(
        repo,
        &env2,
        &contract2,
        "git",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env2.base_ref.clone(), task_id: env2.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("rep2");
    assert!(!rep2.checked_ok);
    assert!(rep2.contract_violations.iter().any(|v| v.contains("git internals")));
}
