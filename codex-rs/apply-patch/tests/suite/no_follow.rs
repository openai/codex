use codex_apply_patch::ApplyPatchOptions;
use codex_apply_patch::MaybeApplyPatchVerified;
use codex_apply_patch::apply_patch_with_options;
use codex_apply_patch::parse_patch;
use codex_apply_patch::verify_apply_patch_args;
use codex_exec_server::LOCAL_FS;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::fs;
use std::os::unix::fs::symlink;

#[tokio::test]
async fn no_follow_rejects_leaf_and_ancestor_links_for_every_patch_operation() -> anyhow::Result<()>
{
    for path in ["link.txt", "linked/victim.txt"] {
        for body in [
            format!("*** Add File: {path}\n+changed"),
            format!("*** Update File: {path}\n@@\n-original\n+changed"),
            format!("*** Delete File: {path}"),
            format!("*** Update File: {path}\n*** Move to: moved.txt\n@@\n-original\n+changed"),
            format!("*** Update File: source.txt\n*** Move to: {path}\n@@\n-original\n+changed"),
            "*** Add File: linked/new/nested.txt\n+changed".to_string(),
            "*** Update File: source.txt\n*** Move to: linked/new/nested.txt\n@@\n-original\n+changed".to_string(),
        ] {
            let temp = tempfile::tempdir()?;
            // macOS's temporary directory can itself contain a /var symlink.
            let root = temp.path().canonicalize()?;
            let work = root.join("work");
            let outside = root.join("outside");
            fs::create_dir(&work)?;
            fs::create_dir(&outside)?;
            fs::write(outside.join("victim.txt"), "original\n")?;
            fs::write(work.join("source.txt"), "original\n")?;
            symlink(outside.join("victim.txt"), work.join("link.txt"))?;
            symlink(&outside, work.join("linked"))?;

            let patch = format!("*** Begin Patch\n{body}\n*** End Patch");
            let failure = apply_patch_with_options(
                &patch,
                ApplyPatchOptions {
                    follow_symlinks: false,
                    ..Default::default()
                },
                &PathUri::from_host_native_path(&work)?,
                &mut Vec::new(),
                &mut Vec::new(),
                LOCAL_FS.as_ref(),
                /*sandbox*/ None,
            )
            .await
            .expect_err(&body);

            assert!(failure.delta().is_empty(), "{body}");
            assert_eq!(fs::read_to_string(outside.join("victim.txt"))?, "original\n");
            assert_eq!(fs::read_to_string(work.join("source.txt"))?, "original\n");
            assert!(!outside.join("new").exists(), "{body}");
            assert!(!work.join("moved.txt").exists(), "{body}");
            assert!(fs::symlink_metadata(work.join("link.txt"))?.is_symlink());
        }
    }
    Ok(())
}

#[tokio::test]
async fn no_follow_rechecks_paths_after_verification() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    fs::create_dir(root.join("approved"))?;
    fs::create_dir(root.join("outside"))?;
    fs::write(root.join("approved/file.txt"), "original\n")?;
    fs::write(root.join("outside/file.txt"), "original\n")?;
    let cwd = PathUri::from_host_native_path(&root)?;
    let patch = "*** Begin Patch\n*** Update File: approved/file.txt\n@@\n-original\n+changed\n*** End Patch";
    let MaybeApplyPatchVerified::Body(action) = verify_apply_patch_args(
        parse_patch(patch)?,
        &cwd,
        LOCAL_FS.as_ref(),
        /*sandbox*/ None,
    )
    .await
    else {
        panic!("regular-file patch should verify");
    };
    fs::rename(root.join("approved"), root.join("original"))?;
    symlink(root.join("outside"), root.join("approved"))?;

    apply_patch_with_options(
        &action.patch,
        ApplyPatchOptions {
            follow_symlinks: false,
            ..Default::default()
        },
        &cwd,
        &mut Vec::new(),
        &mut Vec::new(),
        LOCAL_FS.as_ref(),
        /*sandbox*/ None,
    )
    .await
    .expect_err("swapped ancestor must not be followed");
    assert_eq!(
        fs::read_to_string(root.join("outside/file.txt"))?,
        "original\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("original/file.txt"))?,
        "original\n"
    );
    Ok(())
}

#[tokio::test]
async fn no_follow_applies_regular_files_and_default_still_follows_links() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let cwd = PathUri::from_host_native_path(&root)?;
    let patch = "*** Begin Patch\n*** Add File: new/file.txt\n+original\n*** Update File: new/file.txt\n*** Move to: moved/file.txt\n@@\n-original\n+changed\n*** Add File: remove.txt\n+remove\n*** Delete File: remove.txt\n*** End Patch";
    apply_patch_with_options(
        patch,
        ApplyPatchOptions {
            follow_symlinks: false,
            ..Default::default()
        },
        &cwd,
        &mut Vec::new(),
        &mut Vec::new(),
        LOCAL_FS.as_ref(),
        /*sandbox*/ None,
    )
    .await?;
    assert_eq!(
        fs::read_to_string(root.join("moved/file.txt"))?,
        "changed\n"
    );
    assert!(!root.join("new/file.txt").exists());
    assert!(!root.join("remove.txt").exists());

    symlink(root.join("moved/file.txt"), root.join("link.txt"))?;
    codex_apply_patch::apply_patch(
        "*** Begin Patch\n*** Update File: link.txt\n@@\n-changed\n+followed\n*** End Patch",
        &cwd,
        &mut Vec::new(),
        &mut Vec::new(),
        LOCAL_FS.as_ref(),
        /*sandbox*/ None,
    )
    .await?;
    assert_eq!(
        fs::read_to_string(root.join("moved/file.txt"))?,
        "followed\n"
    );
    Ok(())
}
