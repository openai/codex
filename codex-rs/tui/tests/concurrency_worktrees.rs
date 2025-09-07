use std::path::Path;
use std::process::Command;
use std::thread;

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
fn concurrent_tasks_use_separate_worktrees_without_collisions() {
    if !have_git() {
        eprintln!("skipping: git not available");
        return;
    }

    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path().to_path_buf();
    assert!(Command::new("git").arg("init").current_dir(&repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(&repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(&repo).status().unwrap().success());
    std::fs::write(repo.join("base.txt"), b"base\n").unwrap();
    assert!(Command::new("git").args(["add", "-A"]).current_dir(&repo).status().unwrap().success());
    assert!(Command::new("git").args(["commit", "-m", "chore: base"]).current_dir(&repo).status().unwrap().success());
    let (_ok, base_sha) = git(&repo, ["rev-parse", "--short", "HEAD"]);
    let base_sha = base_sha.trim().to_string();

    // Two independent diffs
    let diff1 = r#"diff --git a/a.txt b/a.txt
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/a.txt
@@ -0,0 +1,1 @@
+A
"#;
    let diff2 = r#"diff --git a/b.txt b/b.txt
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/b.txt
@@ -0,0 +1,1 @@
+B
"#;

    let env1 = DiffEnvelope { base_ref: base_sha.clone(), task_id: "TASK-C1".into(), rationale: "c1".into(), diff: diff1.into() };
    let env2 = DiffEnvelope { base_ref: base_sha.clone(), task_id: "TASK-C2".into(), rationale: "c2".into(), diff: diff2.into() };
    let mut contract1 = ChangeContract::default();
    contract1.task_id = env1.task_id.clone();
    contract1.allowed_paths = vec!["*.txt".into(), "**/*.txt".into()];
    contract1.max_files_changed = Some(2);
    contract1.max_lines_added = Some(10);
    contract1.max_lines_removed = Some(0);
    contract1.allow_renames = false;
    contract1.allow_deletes = false;
    contract1.forbid_binary = true;
    contract1.require_tests = false;
    contract1.commit_prefix = "chore".into();
    let mut contract2 = ChangeContract::default();
    contract2.task_id = env2.task_id.clone();
    contract2.allowed_paths = contract1.allowed_paths.clone();
    contract2.max_files_changed = contract1.max_files_changed;
    contract2.max_lines_added = contract1.max_lines_added;
    contract2.max_lines_removed = contract1.max_lines_removed;
    contract2.commit_prefix = contract1.commit_prefix.clone();

    let r1 = repo.clone();
    let h1 = thread::spawn(move || -> ApplyReport {
        verify_and_apply_patch(
            &r1,
            &env1,
            &contract1,
            "Add a.txt",
            false,
            WorktreePolicy::EphemeralFromBaseRef { base_ref: env1.base_ref.clone(), task_id: env1.task_id.clone() },
            Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
        )
        .expect("apply 1")
    });

    let r2 = repo.clone();
    let h2 = thread::spawn(move || -> ApplyReport {
        verify_and_apply_patch(
            &r2,
            &env2,
            &contract2,
            "Add b.txt",
            false,
            WorktreePolicy::EphemeralFromBaseRef { base_ref: env2.base_ref.clone(), task_id: env2.task_id.clone() },
            Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
        )
        .expect("apply 2")
    });

    let rep1 = h1.join().unwrap();
    let rep2 = h2.join().unwrap();

    assert!(rep1.applied && rep1.committed);
    assert!(rep2.applied && rep2.committed);

    // both branches exist and differ from base
    let (_ok, sha1) = git(&repo, ["rev-parse", "--short", "autopilot/TASK-C1"]);
    let (_ok, sha2) = git(&repo, ["rev-parse", "--short", "autopilot/TASK-C2"]);
    let sha1 = sha1.trim();
    let sha2 = sha2.trim();
    assert_ne!(sha1, base_sha);
    assert_ne!(sha2, base_sha);

    // Ephemeral directories are removed
    assert!(!repo.join(".worktrees").join("autopilot").join("TASK-C1").exists());
    assert!(!repo.join(".worktrees").join("autopilot").join("TASK-C2").exists());
}
