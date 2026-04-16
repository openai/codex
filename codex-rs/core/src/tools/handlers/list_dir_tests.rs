use super::*;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[tokio::test]
async fn lists_directory_entries() {
    let temp = tempdir().expect("create tempdir");
    let dir_path = temp.path();

    let sub_dir = dir_path.join("nested");
    tokio::fs::create_dir(&sub_dir)
        .await
        .expect("create sub dir");

    let deeper_dir = sub_dir.join("deeper");
    tokio::fs::create_dir(&deeper_dir)
        .await
        .expect("create deeper dir");

    tokio::fs::write(dir_path.join("entry.txt"), b"content")
        .await
        .expect("write file");
    tokio::fs::write(sub_dir.join("child.txt"), b"child")
        .await
        .expect("write child");
    tokio::fs::write(deeper_dir.join("grandchild.txt"), b"grandchild")
        .await
        .expect("write grandchild");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let link_path = dir_path.join("link");
        symlink(dir_path.join("entry.txt"), &link_path).expect("create symlink");
    }

    let entries = list_dir_slice(
        dir_path, /*offset*/ 1, /*limit*/ 20, /*depth*/ 3,
    )
    .await
    .expect("list directory");

    #[cfg(unix)]
    let expected = vec![
        "entry.txt".to_string(),
        "link@".to_string(),
        "nested/".to_string(),
        "  child.txt".to_string(),
        "  deeper/".to_string(),
        "    grandchild.txt".to_string(),
    ];

    #[cfg(not(unix))]
    let expected = vec![
        "entry.txt".to_string(),
        "nested/".to_string(),
        "  child.txt".to_string(),
        "  deeper/".to_string(),
        "    grandchild.txt".to_string(),
    ];

    assert_eq!(entries, expected);
}

#[tokio::test]
async fn errors_when_offset_exceeds_entries() {
    let temp = tempdir().expect("create tempdir");
    let dir_path = temp.path();
    tokio::fs::create_dir(dir_path.join("nested"))
        .await
        .expect("create sub dir");

    let err = list_dir_slice(
        dir_path, /*offset*/ 10, /*limit*/ 1, /*depth*/ 2,
    )
    .await
    .expect_err("offset exceeds entries");
    assert_eq!(
        err,
        FunctionCallError::RespondToModel("offset exceeds directory entry count".to_string())
    );
}

#[tokio::test]
async fn respects_depth_parameter() {
    let temp = tempdir().expect("create tempdir");
    let dir_path = temp.path();
    let nested = dir_path.join("nested");
    let deeper = nested.join("deeper");
    tokio::fs::create_dir(&nested).await.expect("create nested");
    tokio::fs::create_dir(&deeper).await.expect("create deeper");
    tokio::fs::write(dir_path.join("root.txt"), b"root")
        .await
        .expect("write root");
    tokio::fs::write(nested.join("child.txt"), b"child")
        .await
        .expect("write nested");
    tokio::fs::write(deeper.join("grandchild.txt"), b"deep")
        .await
        .expect("write deeper");

    let entries_depth_one = list_dir_slice(
        dir_path, /*offset*/ 1, /*limit*/ 10, /*depth*/ 1,
    )
    .await
    .expect("list depth 1");
    assert_eq!(
        entries_depth_one,
        vec!["nested/".to_string(), "root.txt".to_string(),]
    );

    let entries_depth_two = list_dir_slice(
        dir_path, /*offset*/ 1, /*limit*/ 20, /*depth*/ 2,
    )
    .await
    .expect("list depth 2");
    assert_eq!(
        entries_depth_two,
        vec![
            "nested/".to_string(),
            "  child.txt".to_string(),
            "  deeper/".to_string(),
            "root.txt".to_string(),
        ]
    );

    let entries_depth_three = list_dir_slice(
        dir_path, /*offset*/ 1, /*limit*/ 30, /*depth*/ 3,
    )
    .await
    .expect("list depth 3");
    assert_eq!(
        entries_depth_three,
        vec![
            "nested/".to_string(),
            "  child.txt".to_string(),
            "  deeper/".to_string(),
            "    grandchild.txt".to_string(),
            "root.txt".to_string(),
        ]
    );
}

#[tokio::test]
async fn paginates_in_sorted_order() {
    let temp = tempdir().expect("create tempdir");
    let dir_path = temp.path();

    let dir_a = dir_path.join("a");
    let dir_b = dir_path.join("b");
    tokio::fs::create_dir(&dir_a).await.expect("create a");
    tokio::fs::create_dir(&dir_b).await.expect("create b");

    tokio::fs::write(dir_a.join("a_child.txt"), b"a")
        .await
        .expect("write a child");
    tokio::fs::write(dir_b.join("b_child.txt"), b"b")
        .await
        .expect("write b child");

    let first_page = list_dir_slice(
        dir_path, /*offset*/ 1, /*limit*/ 2, /*depth*/ 2,
    )
    .await
    .expect("list page one");
    assert_eq!(
        first_page,
        vec![
            "a/".to_string(),
            "  a_child.txt".to_string(),
            "More than 2 entries found".to_string()
        ]
    );

    let second_page = list_dir_slice(
        dir_path, /*offset*/ 3, /*limit*/ 2, /*depth*/ 2,
    )
    .await
    .expect("list page two");
    assert_eq!(
        second_page,
        vec!["b/".to_string(), "  b_child.txt".to_string()]
    );
}

#[tokio::test]
async fn handles_large_limit_without_overflow() {
    let temp = tempdir().expect("create tempdir");
    let dir_path = temp.path();
    tokio::fs::write(dir_path.join("alpha.txt"), b"alpha")
        .await
        .expect("write alpha");
    tokio::fs::write(dir_path.join("beta.txt"), b"beta")
        .await
        .expect("write beta");
    tokio::fs::write(dir_path.join("gamma.txt"), b"gamma")
        .await
        .expect("write gamma");

    let entries = list_dir_slice(dir_path, /*offset*/ 2, usize::MAX, /*depth*/ 1)
        .await
        .expect("list without overflow");
    assert_eq!(
        entries,
        vec!["beta.txt".to_string(), "gamma.txt".to_string(),]
    );
}

#[tokio::test]
async fn indicates_truncated_results() {
    let temp = tempdir().expect("create tempdir");
    let dir_path = temp.path();

    for idx in 0..40 {
        let file = dir_path.join(format!("file_{idx:02}.txt"));
        tokio::fs::write(file, b"content")
            .await
            .expect("write file");
    }

    let entries = list_dir_slice(
        dir_path, /*offset*/ 1, /*limit*/ 25, /*depth*/ 1,
    )
    .await
    .expect("list directory");
    assert_eq!(entries.len(), 26);
    assert_eq!(
        entries.last(),
        Some(&"More than 25 entries found".to_string())
    );
}

#[tokio::test]
async fn truncation_respects_sorted_order() -> anyhow::Result<()> {
    let temp = tempdir()?;
    let dir_path = temp.path();
    let nested = dir_path.join("nested");
    let deeper = nested.join("deeper");
    tokio::fs::create_dir(&nested).await?;
    tokio::fs::create_dir(&deeper).await?;
    tokio::fs::write(dir_path.join("root.txt"), b"root").await?;
    tokio::fs::write(nested.join("child.txt"), b"child").await?;
    tokio::fs::write(deeper.join("grandchild.txt"), b"deep").await?;

    let entries_depth_three = list_dir_slice(
        dir_path, /*offset*/ 1, /*limit*/ 3, /*depth*/ 3,
    )
    .await?;
    assert_eq!(
        entries_depth_three,
        vec![
            "nested/".to_string(),
            "  child.txt".to_string(),
            "  deeper/".to_string(),
            "More than 3 entries found".to_string()
        ]
    );

    Ok(())
}

#[tokio::test]
async fn hides_denied_entries_and_prunes_denied_subtrees() {
    let temp = tempdir().expect("create tempdir");
    let dir_path = temp.path();
    let visible_dir = dir_path.join("visible");
    let denied_dir = dir_path.join("private");
    tokio::fs::create_dir(&visible_dir)
        .await
        .expect("create visible dir");
    tokio::fs::create_dir(&denied_dir)
        .await
        .expect("create denied dir");
    tokio::fs::write(visible_dir.join("ok.txt"), b"ok")
        .await
        .expect("write visible file");
    tokio::fs::write(denied_dir.join("secret.txt"), b"secret")
        .await
        .expect("write denied file");
    tokio::fs::write(dir_path.join("top_secret.txt"), b"secret")
        .await
        .expect("write denied top-level file");

    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: denied_dir.try_into().expect("absolute denied dir"),
            },
            access: FileSystemAccessMode::None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: dir_path
                    .join("top_secret.txt")
                    .try_into()
                    .expect("absolute denied file"),
            },
            access: FileSystemAccessMode::None,
        },
    ]);

    let read_deny_matcher = ReadDenyMatcher::new(&policy, dir_path);
    let entries = list_dir_slice_with_policy(
        dir_path,
        /*offset*/ 1,
        /*limit*/ 20,
        /*depth*/ 3,
        read_deny_matcher.as_ref(),
    )
    .await
    .expect("list directory");

    assert_eq!(
        entries,
        vec!["visible/".to_string(), "  ok.txt".to_string(),]
    );
}
