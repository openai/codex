use std::path::Path;
use std::process::Command;

use codex_tui::git_guard::{verify_and_apply_patch, DiffEnvelope, WorktreePolicy};
use codex_tui::change_contract::ChangeContract;

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

#[test]
fn deny_presets_block_paths_via_config() {
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

    // Write .autopilot/config.toml
    let ap = repo.join(".autopilot");
    std::fs::create_dir_all(&ap).unwrap();
    std::fs::write(ap.join("config.toml"), b"deny_presets = [\"node_modules\", \"dist\"]\n").unwrap();

    // Touch a forbidden path via preset
    let diff = r#"diff --git a/node_modules/pkg/index.js b/node_modules/pkg/index.js
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/node_modules/pkg/index.js
@@ -0,0 +1 @@
+console.log(1)
"#;
    let env = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-PRESET".into(), rationale: "preset".into(), diff: diff.into() };
    let mut contract = ChangeContract::default();
    contract.task_id = env.task_id.clone();
    // empty allowed_paths => allow-all

    let rep = verify_and_apply_patch(
        repo,
        &env,
        &contract,
        "preset",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env.base_ref.clone(), task_id: env.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("rep");
    assert!(!rep.checked_ok);
    assert!(rep.contract_violations.iter().any(|v| v.contains("preset")));
}

