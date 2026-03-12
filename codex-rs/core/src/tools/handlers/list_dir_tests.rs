use super::*;
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

    let entries = list_dir_slice(dir_path, 1, 20, 3)
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

    let err = list_dir_slice(dir_path, 10, 1, 2)
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

    let entries_depth_one = list_dir_slice(dir_path, 1, 10, 1)
        .await
        .expect("list depth 1");
    assert_eq!(
        entries_depth_one,
        vec!["nested/".to_string(), "root.txt".to_string(),]
    );

    let entries_depth_two = list_dir_slice(dir_path, 1, 20, 2)
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

    let entries_depth_three = list_dir_slice(dir_path, 1, 30, 3)
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

    let first_page = list_dir_slice(dir_path, 1, 2, 2)
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

    let second_page = list_dir_slice(dir_path, 3, 2, 2)
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

    let entries = list_dir_slice(dir_path, 2, usize::MAX, 1)
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

    let entries = list_dir_slice(dir_path, 1, 25, 1)
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

    let entries_depth_three = list_dir_slice(dir_path, 1, 3, 3).await?;
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
