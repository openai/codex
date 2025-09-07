use std::path::Path;
use std::process::Command;

use codex_tui::git_guard::{verify_and_apply_patch, DiffEnvelope, WorktreePolicy};
use codex_tui::change_contract::ChangeContract;

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

#[test]
fn detects_pem_and_tokens_and_minified() {
    if !have_git() {
        eprintln!("skipping: git not available");
        return;
    }

    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path();
    assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["commit", "--allow-empty", "-m", "base"]).current_dir(repo).status().unwrap().success());

    // 1) PEM private key pattern
    let diff_pem = r#"diff --git a/secret.pem b/secret.pem
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/secret.pem
@@ -0,0 +1,3 @@
+-----BEGIN PRIVATE KEY-----
+abc
+-----END PRIVATE KEY-----
"#;
    let env1 = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-SEC1".into(), rationale: "pem".into(), diff: diff_pem.into() };
    let mut contract = ChangeContract::default();
    contract.task_id = env1.task_id.clone();
    contract.allowed_paths = vec!["**/*.pem".into(), "secret.pem".into()];
    let rep1 = verify_and_apply_patch(
        repo,
        &env1,
        &contract,
        "pem",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env1.base_ref.clone(), task_id: env1.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("rep1");
    assert!(!rep1.checked_ok);
    assert!(rep1.contract_violations.iter().any(|v| v.contains("suspected secret")));

    // 2) GitHub PAT + Google API key + minified line
    let long = "a".repeat(1200);
    let diff_tok = format!(
        "diff --git a/app.js b/app.js\nnew file mode 100644\nindex 0000000..e69de29\n--- /dev/null\n+++ b/app.js\n@@ -0,0 +1,3 @@\n+ghp_0123456789abcdefghijklmnopqrstuvwx\n+AIzaabcdefghijklmnopqrstuvwx0123456789abc\n+{}\n",
        long
    );
    let env2 = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-SEC2".into(), rationale: "tokens+minified".into(), diff: diff_tok };
    let mut contract2 = ChangeContract::default();
    contract2.task_id = env2.task_id.clone();
    let rep2 = verify_and_apply_patch(
        repo,
        &env2,
        &contract2,
        "tok+mini",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env2.base_ref.clone(), task_id: env2.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("rep2");
    assert!(!rep2.checked_ok);
    assert!(rep2.contract_violations.iter().any(|v| v.contains("suspected secret")));
    assert!(rep2.contract_violations.iter().any(|v| v.contains("minified-like")));
}

