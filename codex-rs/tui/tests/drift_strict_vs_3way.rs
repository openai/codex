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
fn strict_fails_but_three_way_succeeds_on_drift() {
    if !have_git() {
        eprintln!("skipping: git not available");
        return;
    }

    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path();
    assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());

    // base commit
    std::fs::write(repo.join("file.txt"), b"one\ntwo\nthree\n").unwrap();
    assert!(Command::new("git").args(["add", "-A"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["commit", "-m", "base"]).current_dir(repo).status().unwrap().success());
    let (_ok, base_sha) = git(repo, ["rev-parse", "--short", "HEAD"]);
    let base_sha = base_sha.trim().to_string();

    // Create a feature branch off base that changes 'two' -> 'twoX'
    assert!(Command::new("git").args(["checkout", "-b", "feature", &base_sha]).current_dir(repo).status().unwrap().success());
    std::fs::write(repo.join("file.txt"), b"one\ntwoX\nthree\n").unwrap();
    assert!(Command::new("git").args(["commit", "-am", "feat: change two -> twoX"]).current_dir(repo).status().unwrap().success());
    let (_ok, feature_sha) = git(repo, ["rev-parse", "--short", "HEAD"]);
    let feature_sha = feature_sha.trim().to_string();

    // Generate a diff from base..feature
    let (_ok, diff) = git(repo, ["diff", &format!("{}..{}", base_sha, feature_sha)]);

    // Checkout back to main and introduce drift that changes context around but not the target line
    assert!(Command::new("git").args(["checkout", "-B", "main", &base_sha]).current_dir(repo).status().unwrap().success());
    std::fs::write(repo.join("file.txt"), b"ZERO\none\ntwo\nthree\n").unwrap();
    assert!(Command::new("git").args(["commit", "-am", "chore: add line above"]).current_dir(repo).status().unwrap().success());

    let env = DiffEnvelope { base_ref: base_sha.clone(), task_id: "TASK-DRIFT".into(), rationale: "drift".into(), diff };
    let mut contract = ChangeContract::default();
    contract.task_id = env.task_id.clone();
    contract.allowed_paths = vec!["file.txt".into()];

    // InPlace so we actually test fallback (ephemeral would be at base_ref and strict would pass)
    let rep = verify_and_apply_patch(
        repo,
        &env,
        &contract,
        "apply drift patch",
        false,
        WorktreePolicy::InPlace,
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("3-way fallback should succeed");
    assert!(rep.applied && rep.committed);
}

