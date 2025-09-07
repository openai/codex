use std::path::Path;
use std::process::Command;

use codex_tui::git_guard::{verify_and_apply_patch, DiffEnvelope, WorktreePolicy};
use codex_tui::change_contract::ChangeContract;

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

#[test]
fn enforces_per_file_budgets_and_new_files() {
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

    // Patch with two new files, one with two hunks and >1 line added
    let diff = r#"diff --git a/a.txt b/a.txt
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/a.txt
@@ -0,0 +1,2 @@
+A1
+A2
diff --git a/b.txt b/b.txt
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/b.txt
@@ -0,0 +1 @@
+B1
@@ -0,0 +1 @@
+B2
"#;

    let env = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-PERFILE".into(), rationale: "per-file budgets".into(), diff: diff.into() };
    let mut contract = ChangeContract::default();
    contract.task_id = env.task_id.clone();
    contract.allowed_paths = vec!["**/*.txt".into()];
    contract.max_new_files = Some(1); // but patch creates 2
    contract.max_lines_added_per_file = Some(1); // a.txt adds 2 lines
    contract.max_hunks_per_file = Some(1); // b.txt has 2 hunks
    contract.max_bytes_per_file = Some(2); // a.txt bytes>2

    let rep = verify_and_apply_patch(
        repo,
        &env,
        &contract,
        "per-file budgets",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env.base_ref.clone(), task_id: env.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("report");
    assert!(!rep.checked_ok);
    assert!(rep.contract_violations.iter().any(|v| v.contains("new_files")));
    assert!(rep.contract_violations.iter().any(|v| v.contains("lines_added")));
    assert!(rep.contract_violations.iter().any(|v| v.contains("hunks")));
    assert!(rep.contract_violations.iter().any(|v| v.contains("bytes_added")));
}
