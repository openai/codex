use std::path::Path;
use std::process::Command;

use codex_tui::git_guard::{verify_and_apply_patch, DiffEnvelope, WorktreePolicy};
use codex_tui::change_contract::ChangeContract;

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

#[test]
fn rejects_exec_bit_and_symlink_when_forbidden() {
    if !have_git() {
        eprintln!("skipping: git not available");
        return;
    }

    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path();
    assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());

    // simple base commit
    std::fs::write(repo.join(".gitignore"), b"/target\n").unwrap();
    assert!(Command::new("git").args(["add", "-A"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["commit", "-m", "chore: init"]).current_dir(repo).status().unwrap().success());

    // 1) Exec-bit new file (100755)
    let diff_exec = r#"diff --git a/run.sh b/run.sh
new file mode 100755
index 0000000..e69de29
--- /dev/null
+++ b/run.sh
@@ -0,0 +1 @@
+echo hi
"#;
    let env1 = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-EXE".into(), rationale: "exec".into(), diff: diff_exec.into() };
    let mut contract = ChangeContract::default();
    contract.task_id = env1.task_id.clone();
    contract.allowed_paths = vec!["**/*.sh".into(), "run.sh".into()];
    contract.forbid_exec_mode_changes = true;
    contract.forbid_symlinks = true;

    let rep = verify_and_apply_patch(
        repo,
        &env1,
        &contract,
        "Add run.sh",
        false,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env1.base_ref.clone(), task_id: env1.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("report");
    assert!(!rep.checked_ok && !rep.applied && !rep.committed);
    assert!(rep.contract_violations.iter().any(|v| v.contains("exec mode")));

    // 2) Symlink new file (120000)
    let diff_sym = r#"diff --git a/link b/link
new file mode 120000
index 0000000..e69de29
--- /dev/null
+++ b/link
@@ -0,0 +1 @@
+target-path
"#;
    let env2 = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-SYM".into(), rationale: "symlink".into(), diff: diff_sym.into() };
    let mut contract2 = ChangeContract::default();
    contract2.task_id = env2.task_id.clone();
    contract2.allowed_paths = vec!["link".into()];
    contract2.forbid_symlinks = true;

    let rep2 = verify_and_apply_patch(
        repo,
        &env2,
        &contract2,
        "Add link",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env2.base_ref.clone(), task_id: env2.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("report2");
    assert!(!rep2.checked_ok);
    assert!(rep2.contract_violations.iter().any(|v| v.contains("symlink")));
}
