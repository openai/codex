use super::*;
use codex_apply_patch::MaybeApplyPatchVerified;
use codex_exec_server::LOCAL_FS;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn diff_consumer_does_not_stream_json_tool_call_arguments() {
    let mut consumer = ApplyPatchArgumentDiffConsumer::default();
    assert!(
        consumer
            .push_delta("call-1".to_string(), r#"{"input":"*** Begin Patch\n"#)
            .is_none()
    );
    assert!(
        consumer
            .push_delta(
                "call-1".to_string(),
                r#"*** Add File: hello.txt\n+hello\n*** End Patch\n"}"#
            )
            .is_none()
    );
}

#[test]
fn diff_consumer_streams_apply_patch_changes() {
    let mut consumer = ApplyPatchArgumentDiffConsumer::default();
    assert!(
        consumer
            .push_delta("call-1".to_string(), "*** Begin Patch\n")
            .is_none()
    );

    let event = consumer
        .push_delta("call-1".to_string(), "*** Add File: hello.txt\n+hello")
        .expect("progress event");
    assert_eq!(
        (event.call_id, event.changes),
        (
            "call-1".to_string(),
            HashMap::from([(
                PathBuf::from("hello.txt"),
                FileChange::Add {
                    content: "hello\n".to_string(),
                },
            )]),
        )
    );

    assert!(
        consumer
            .push_delta("call-1".to_string(), "\n+world")
            .is_none()
    );

    let event = consumer.flush_update_on_complete().expect("progress event");
    assert_eq!(
        (event.call_id, event.changes),
        (
            "call-1".to_string(),
            HashMap::from([(
                PathBuf::from("hello.txt"),
                FileChange::Add {
                    content: "hello\nworld\n".to_string(),
                },
            )]),
        )
    );
}

#[test]
fn diff_consumer_sends_next_update_after_buffer_interval() {
    let mut consumer = ApplyPatchArgumentDiffConsumer::default();
    consumer.push_delta("call-1".to_string(), "*** Begin Patch\n");
    let first = consumer
        .push_delta("call-1".to_string(), "*** Add File: hello.txt\n+hello")
        .expect("first progress event");
    assert_eq!(
        first.changes,
        HashMap::from([(
            PathBuf::from("hello.txt"),
            FileChange::Add {
                content: "hello\n".to_string(),
            },
        )])
    );

    consumer.last_sent_at =
        Some(std::time::Instant::now() - APPLY_PATCH_ARGUMENT_DIFF_BUFFER_INTERVAL);
    let second = consumer
        .push_delta("call-1".to_string(), "\n+world")
        .expect("second progress event");
    assert_eq!(
        second.changes,
        HashMap::from([(
            PathBuf::from("hello.txt"),
            FileChange::Add {
                content: "hello\nworld\n".to_string(),
            },
        )])
    );
}

#[test]
fn split_freeform_apply_patch_environment_extracts_environment_header() {
    let input =
        "*** Environment: local\n*** Begin Patch\n*** Add File: hello.txt\n+hello\n*** End Patch\n"
            .to_string();

    let (patch, environment_id) =
        split_freeform_apply_patch_environment(input).expect("header should parse");

    assert_eq!(environment_id.as_deref(), Some("local"));
    assert_eq!(
        patch,
        "*** Begin Patch\n*** Add File: hello.txt\n+hello\n*** End Patch\n"
    );
}

#[test]
fn merge_apply_patch_environment_ids_rejects_mismatch() {
    let err =
        merge_apply_patch_environment_ids(Some("local".to_string()), Some("remote".to_string()))
            .expect_err("mismatched environment ids should fail");

    assert!(err.to_string().contains("environment id mismatch"));
}

#[tokio::test]
async fn approval_keys_include_move_destination() {
    let tmp = TempDir::new().expect("tmp");
    let cwd_path = tmp.path();
    let cwd = cwd_path.abs();
    std::fs::create_dir_all(cwd_path.join("old")).expect("create old dir");
    std::fs::create_dir_all(cwd_path.join("renamed/dir")).expect("create dest dir");
    std::fs::write(cwd_path.join("old/name.txt"), "old content\n").expect("write old file");
    let patch = r#"*** Begin Patch
*** Update File: old/name.txt
*** Move to: renamed/dir/name.txt
@@
-old content
+new content
*** End Patch"#;
    let argv = vec!["apply_patch".to_string(), patch.to_string()];
    let action = match codex_apply_patch::maybe_parse_apply_patch_verified(
        &argv,
        &cwd,
        LOCAL_FS.as_ref(),
        /*sandbox*/ None,
    )
    .await
    {
        MaybeApplyPatchVerified::Body(action) => action,
        other => panic!("expected patch body, got: {other:?}"),
    };

    let keys = file_paths_for_action(&action);
    assert_eq!(keys.len(), 2);
}

#[test]
fn write_permissions_for_paths_skip_dirs_already_writable_under_workspace_root() {
    let tmp = TempDir::new().expect("tmp");
    let cwd_path = tmp.path();
    let cwd = cwd_path.abs();
    let nested = cwd_path.join("nested");
    std::fs::create_dir_all(&nested).expect("create nested dir");
    let file_path = AbsolutePathBuf::try_from(nested.join("file.txt"))
        .expect("nested file path should be absolute");
    let sandbox_policy = FileSystemSandboxPolicy::from(&SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: false,
    });

    let permissions = write_permissions_for_paths(&[file_path], &sandbox_policy, &cwd);

    assert_eq!(permissions, None);
}

#[test]
fn write_permissions_for_paths_keep_dirs_outside_workspace_root() {
    let tmp = TempDir::new().expect("tmp");
    let cwd = tmp.path().join("workspace");
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&cwd).expect("create cwd");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    let file_path = AbsolutePathBuf::try_from(outside.join("file.txt"))
        .expect("outside file path should be absolute");
    let cwd_abs = cwd.abs();
    let sandbox_policy = FileSystemSandboxPolicy::from(&SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    });

    let permissions = write_permissions_for_paths(&[file_path], &sandbox_policy, &cwd_abs);
    let expected_outside =
        dunce::simplified(&outside.canonicalize().expect("canonicalize outside dir")).abs();

    assert_eq!(
        permissions
            .and_then(|profile| profile.file_system)
            .and_then(|fs| fs.legacy_read_write_roots())
            .and_then(|(_read, write)| write),
        Some(vec![expected_outside])
    );
}
