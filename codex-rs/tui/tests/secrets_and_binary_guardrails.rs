use std::path::Path;
use std::process::Command;

use codex_tui::git_guard::{verify_and_apply_patch, DiffEnvelope, WorktreePolicy};
use codex_tui::change_contract::ChangeContract;

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

#[test]
fn rejects_binary_patch_when_forbidden() {
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

    // Simulate a binary patch via header
    let diff = r#"diff --git a/bin.dat b/bin.dat
new file mode 100644
index 0000000..e69de29
Binary files /dev/null and b/bin.dat differ
"#;
    let env = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-BIN".into(), rationale: "bin".into(), diff: diff.into() };
    let mut contract = ChangeContract::default();
    contract.task_id = env.task_id.clone();
    contract.forbid_binary = true;

    let rep = verify_and_apply_patch(
        repo,
        &env,
        &contract,
        "bin",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env.base_ref.clone(), task_id: env.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("rep");
    assert!(!rep.checked_ok);
    assert!(rep.contract_violations.iter().any(|v| v.contains("binary")));
}
