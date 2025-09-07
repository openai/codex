use codex_tui::patch_mode::enforce_diff_envelope_or_err;
use std::path::Path;
use std::process::Command;
use codex_tui::git_guard::{verify_and_apply_patch, WorktreePolicy};
use codex_tui::change_contract::ChangeContract;

#[test]
fn accept_rename_only() {
    let body = r#"
<diff_envelope>
base_ref: main
task_id: T-rename-ok
rationale: "rename"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/old.txt b/new.txt
similarity index 100%
rename from old.txt
rename to new.txt
---END DIFF---
</diff_envelope>
"#;
    let env = enforce_diff_envelope_or_err(body).expect("env");
    assert_eq!(env.task_id, "T-rename-ok");
}

#[test]
fn accept_copy_only() {
    let body = r#"
<diff_envelope>
base_ref: main
task_id: T-copy-ok
rationale: "copy"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/src.rs b/src_copy.rs
similarity index 100%
copy from src.rs
copy to src_copy.rs
---END DIFF---
</diff_envelope>
"#;
    let env = enforce_diff_envelope_or_err(body).expect("env");
    assert_eq!(env.task_id, "T-copy-ok");
}

#[test]
fn reject_rename_missing_to() {
    let body = r#"
<diff_envelope>
base_ref: main
task_id: T-rename-bad
rationale: "rename"
diff_format: "unified"
---BEGIN DIFF---
similarity index 100%
rename from old.txt
---END DIFF---
</diff_envelope>
"#;
    let err = enforce_diff_envelope_or_err(body).unwrap_err();
    assert_eq!(
        format!("{err}"),
        "envelope diff must be unified (diff --git) or rename/copy-only with similarity index and from/to"
    );
}

#[test]
fn reject_copy_missing_similarity() {
    let body = r#"
<diff_envelope>
base_ref: main
task_id: T-copy-bad
rationale: "copy"
diff_format: "unified"
---BEGIN DIFF---
copy from src.rs
copy to src_copy.rs
---END DIFF---
</diff_envelope>
"#;
    let err = enforce_diff_envelope_or_err(body).unwrap_err();
    assert_eq!(
        format!("{err}"),
        "envelope diff must be unified (diff --git) or rename/copy-only with similarity index and from/to"
    );
}

#[test]
fn contract_forbids_rename_and_copy_when_flags_false() {
    // Setup repo
    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path();
    assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["commit", "--allow-empty", "-m", "base"]).current_dir(repo).status().unwrap().success());

    // Rename-only envelope
    let rename_env = r#"
<diff_envelope>
base_ref: HEAD
task_id: T-rc-deny
rationale: "rename"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/old.txt b/new.txt
similarity index 100%
rename from old.txt
rename to new.txt
---END DIFF---
</diff_envelope>
"#;
    let env = enforce_diff_envelope_or_err(rename_env).expect("env");
    let mut contract = ChangeContract::default();
    contract.task_id = env.task_id.clone();
    contract.allow_renames = false;
    contract.allow_copies = false;
    let rep = verify_and_apply_patch(
        repo,
        &env,
        &contract,
        "subject",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env.base_ref.clone(), task_id: env.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("report");
    assert!(!rep.checked_ok);
    assert!(rep.contract_violations.iter().any(|v| v.contains("renames are not allowed")));

    // Copy-only envelope
    let copy_env = r#"
<diff_envelope>
base_ref: HEAD
task_id: T-rc-deny
rationale: "copy"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/a b/b
similarity index 100%
copy from a
copy to b
---END DIFF---
</diff_envelope>
"#;
    let env2 = enforce_diff_envelope_or_err(copy_env).expect("env2");
    let rep2 = verify_and_apply_patch(
        repo,
        &env2,
        &contract,
        "subject",
        true,
        WorktreePolicy::EphemeralFromBaseRef { base_ref: env2.base_ref.clone(), task_id: env2.task_id.clone() },
        Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
    ).expect("report2");
    assert!(!rep2.checked_ok);
    assert!(rep2.contract_violations.iter().any(|v| v.contains("copies are not allowed")));
}
