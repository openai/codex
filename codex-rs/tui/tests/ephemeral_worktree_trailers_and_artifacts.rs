use std::fs;
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
fn trailers_and_artifacts_persisted() {
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

    let diff_body = r#"diff --git a/README.md b/README.md
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/README.md
@@ -0,0 +1 @@
+ok
"#;
    let env = DiffEnvelope { base_ref: "HEAD".into(), task_id: "TASK-P2".into(), rationale: "trailers".into(), diff: diff_body.into() };
    let mut contract = ChangeContract::default();
    contract.task_id = env.task_id.clone();
    contract.allowed_paths = vec!["README.md".into()];

    let _rep = verify_and_apply_patch(
        repo,
        &env,
        &contract,
        "Add README",
        false,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env.base_ref.clone(), task_id: env.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("apply commit");

    // Verify trailers in commit message
    let (_ok, msg) = git(repo, ["log", "-1", "--pretty=%B", "autopilot/TASK-P2"]);
    assert!(msg.contains("PRD-Ref:"));
    assert!(msg.contains("Contract-Hash:"));
    assert!(msg.contains("Diff-Hash:"));
    assert!(msg.contains("Task-Id: TASK-P2"));

    // Verify artifacts written
    let base = repo.join(".autopilot").join("rollouts").join("TASK-P2");
    let entries = fs::read_dir(&base).expect("rollouts dir");
    let mut found = None;
    for e in entries {
        let p = e.unwrap().path();
        if p.is_dir() {
            let envp = p.join("envelope.json");
            let conp = p.join("contract.json");
            let repp = p.join("report.json");
            if envp.exists() && conp.exists() && repp.exists() {
                found = Some((envp, conp, repp));
                break;
            }
        }
    }
    let (envp, conp, repp) = found.expect("artifact triplet");
    let envj: serde_json::Value = serde_json::from_slice(&fs::read(envp).unwrap()).unwrap();
    let conj: serde_json::Value = serde_json::from_slice(&fs::read(conp).unwrap()).unwrap();
    let repj: serde_json::Value = serde_json::from_slice(&fs::read(repp).unwrap()).unwrap();
    assert_eq!(envj["task_id"], "TASK-P2");
    assert_eq!(conj["task_id"], "TASK-P2");
    assert_eq!(repj["task_id"], "TASK-P2");
}
