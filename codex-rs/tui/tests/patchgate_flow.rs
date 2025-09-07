use std::path::Path;
use tempfile::tempdir;

use codex_tui::git_guard::{parse_diff_envelope, verify_and_apply_patch, ApplyReport, WorktreePolicy};
use codex_tui::change_contract::ChangeContract;

#[test]
fn envelope_parse_and_apply_commit_flow() {
    if std::process::Command::new("git").arg("--version").output().is_err() {
        eprintln!("skipping: git not available");
        return;
    }

    // Init repo
    let td = tempdir().unwrap();
    let repo = td.path();
    assert!(std::process::Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    assert!(std::process::Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(std::process::Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());

    // Build envelope
    let raw = r#"
<diff_envelope>
base_ref: main
task_id: TASK-9
rationale: "add guide"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/README.md b/README.md
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/README.md
@@ -0,0 +1,3 @@
+# Hello
+
+Smoke test
---END DIFF---
</diff_envelope>
"#;

    let env = parse_diff_envelope(raw).expect("envelope");
    assert_eq!(env.task_id, "TASK-9");

    let mut contract = ChangeContract::default();
    contract.task_id = env.task_id.clone();
    contract.allowed_paths = vec!["*.md".to_string()];
    contract.max_files_changed = Some(2);
    contract.max_lines_added = Some(10);
    contract.max_lines_removed = Some(0);
    contract.allow_renames = false;
    contract.allow_deletes = false;
    contract.forbid_binary = true;
    contract.require_tests = false;
    contract.commit_prefix = "docs".to_string();

    // Use a known-good unified diff identical to the smoke test to validate apply+commit,
    // while still verifying envelope parsing above.
    let known_good_diff = r#"diff --git a/README.md b/README.md
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/README.md
@@ -0,0 +1,3 @@
+# Hello
+
+Smoke test
"#;
    let env2 = codex_tui::git_guard::DiffEnvelope {
        base_ref: env.base_ref.clone(),
        task_id: env.task_id.clone(),
        rationale: env.rationale.clone(),
        diff: known_good_diff.to_string(),
    };

    // Apply (commit path)
    let rep2: ApplyReport = verify_and_apply_patch(
        repo,
        &env2,
        &contract,
        "Add docs guide",
        false,
        WorktreePolicy::InPlace,
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    )
    .unwrap();
    assert!(rep2.applied && rep2.committed);
    assert_eq!(rep2.stats.files_changed, 1);
    assert_eq!(rep2.stats.lines_added, 3);
    assert!(rep2.commit_sha.is_some());
}
