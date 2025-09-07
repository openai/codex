use codex_tui::patch_mode::enforce_diff_envelope_or_err;

#[test]
fn accept_valid_unified_envelope() {
    let body = r#"
<diff_envelope>
base_ref: main
task_id: T-1
rationale: "ok"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/README.md b/README.md
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/README.md
@@ -0,0 +1 @@
+ok
---END DIFF---
</diff_envelope>
"#;
    let env = enforce_diff_envelope_or_err(body).expect("env");
    assert_eq!(env.base_ref, "main");
    assert_eq!(env.task_id, "T-1");
    assert!(env.diff.contains("diff --git "));
}

#[test]
fn accept_rename_only_envelope() {
    let body = r#"
<diff_envelope>
base_ref: main
task_id: T-rename
rationale: "rename only"
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
    assert_eq!(env.task_id, "T-rename");
}

#[test]
fn fenced_envelope_only_is_accepted() {
    let body = r#"
```
<diff_envelope>
base_ref: main
task_id: T-2
rationale: "bad"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/a b/a
---END DIFF---
</diff_envelope>
```
"#;
    let env = enforce_diff_envelope_or_err(body).expect("env");
    assert_eq!(env.task_id, "T-2");
}

#[test]
fn reject_extra_prose_before_after() {
    let body = r#"
hello
<diff_envelope>
base_ref: main
task_id: T-3
rationale: "bad"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/a b/a
---END DIFF---
</diff_envelope>
world
"#;
    let err = enforce_diff_envelope_or_err(body).unwrap_err();
    assert_eq!(format!("{err}"), "output contains extra content outside the envelope");
}

#[test]
fn accept_copy_only_envelope() {
    let body = r#"
<diff_envelope>
base_ref: main
task_id: T-copy
rationale: "copy only"
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
    assert_eq!(env.task_id, "T-copy");
}

#[test]
fn fenced_unwrapped_envelope_ok() {
    let body = r#"
```
<diff_envelope>
base_ref: main
task_id: T-fenced
rationale: "ok"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/a b/a
deleted file mode 100644
---END DIFF---
</diff_envelope>
```
"#;
    let env = enforce_diff_envelope_or_err(body).expect("env");
    assert_eq!(env.task_id, "T-fenced");
}

#[test]
fn delete_only_and_mode_only_envelopes_ok() {
    // delete-only (has diff --git)
    let del = r#"
<diff_envelope>
base_ref: main
task_id: T-del
rationale: "del"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/a b/a
deleted file mode 100644
---END DIFF---
</diff_envelope>
"#;
    enforce_diff_envelope_or_err(del).expect("delete ok");

    // mode-only (has diff --git old/new mode)
    let mode = r#"
<diff_envelope>
base_ref: main
task_id: T-mode
rationale: "mode"
diff_format: "unified"
---BEGIN DIFF---
diff --git a/b b/b
old mode 100644
new mode 100755
---END DIFF---
</diff_envelope>
"#;
    enforce_diff_envelope_or_err(mode).expect("mode ok");
}

#[test]
fn reject_mixed_envelope() {
    // Contains similarity index but missing full rename/copy pairs and no diff --git
    let body = r#"
<diff_envelope>
base_ref: main
task_id: T-mixed
rationale: "mixed"
diff_format: "unified"
---BEGIN DIFF---
similarity index 75%
rename from a
random text here
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
fn invalid_body_rejected_by_enforce() {
    use std::path::Path;
    use std::process::Command;
    use codex_tui::git_guard::{verify_and_apply_patch, WorktreePolicy};
    use codex_tui::change_contract::ChangeContract;

    // Init minimal repo
    let td = tempfile::tempdir().expect("tempdir");
    let repo = td.path();
    assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["config", "user.name", "Codex Test"]).current_dir(repo).status().unwrap().success());
    assert!(Command::new("git").args(["commit", "--allow-empty", "-m", "base"]).current_dir(repo).status().unwrap().success());

    // Envelope with invalid body (no headers)
    let body = r#"
<diff_envelope>
base_ref: HEAD
task_id: T-bad
rationale: "bad body"
diff_format: "unified"
---BEGIN DIFF---
this is not a diff
---END DIFF---
</diff_envelope>
"#;
    let err = enforce_diff_envelope_or_err(body).unwrap_err();
    assert_eq!(
        format!("{err}"),
        "envelope diff must be unified (diff --git) or rename/copy-only with similarity index and from/to"
    );
}
