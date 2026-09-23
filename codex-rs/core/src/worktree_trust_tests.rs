use super::MetadataOverrideFileSystem;
use super::create_test_git_repo;
use super::resolve_root_git_project_for_trust;
use super::write_linked_worktree_metadata;
use codex_exec_server::LOCAL_FS;
use codex_git_utils::resolve_root_git_project_uri_for_trust;
#[cfg(unix)]
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path::normalize_for_path_comparison;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::TempDir;
use tokio::process::Command;

#[tokio::test]
async fn portable_trust_roots_support_posix_windows_drive_and_unc_paths() {
    for root in [
        "file:///repo%20with%20spaces",
        "file:///C:/Repo%20%23",
        "file://server/share/Repo%20%23",
    ] {
        let root = PathUri::parse(root).unwrap();
        let checkout = root.parent().unwrap().join("checkout").unwrap();
        let dot_git = root.join(".git").unwrap();
        let git_dir = dot_git.join("worktrees/dev").unwrap();
        let checkout_dot_git = checkout.join(".git").unwrap();
        let fs = MetadataOverrideFileSystem::with_entries([
            (root.join("src").unwrap(), None),
            (dot_git.clone(), None),
            (
                dot_git.join("HEAD").unwrap(),
                Some("ref: refs/heads/main\n".into()),
            ),
            (checkout.clone(), None),
            (checkout.join("src").unwrap(), None),
            (git_dir.clone(), None),
            (
                checkout_dot_git.clone(),
                Some(format!(
                    "gitdir: {}\n",
                    git_dir.inferred_native_path_string()
                )),
            ),
            (
                git_dir.join("gitdir").unwrap(),
                Some(checkout_dot_git.inferred_native_path_string()),
            ),
            (git_dir.join("commondir").unwrap(), Some("../..".into())),
        ]);
        for cwd in [root.join("src").unwrap(), checkout.join("src").unwrap()] {
            assert_eq!(
                resolve_root_git_project_uri_for_trust(&fs, &cwd)
                    .await
                    .map(|path| path.to_url()),
                Some(root.to_url().clone()),
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn opaque_uri_ancestry_remains_available_only_to_native_trust_lookup() {
    // This POSIX spelling would imply a Windows drive in an ordinary URI.
    let root = AbsolutePathBuf::try_from("/C:/project").unwrap();
    let cwd = root.join("src");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    assert!(cwd_uri.is_opaque());
    let fs = MetadataOverrideFileSystem::with_entries([
        (cwd_uri.clone(), None),
        (PathUri::from_abs_path(&root.join(".git")), None),
        (
            PathUri::from_abs_path(&root.join(".git/HEAD")),
            Some("ref: refs/heads/main\n".into()),
        ),
    ]);
    assert_eq!(
        resolve_root_git_project_for_trust(&fs, &cwd).await,
        Some(root)
    );
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_validates_main_checkout_ownership() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let checkout = tmp.path().join("checkout");
    let admin = write_linked_worktree_metadata(&repo, &checkout);
    fs::write(admin.join("gitdir"), "../../../../checkout/.git\n").unwrap();
    assert_eq!(
        resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &checkout.abs()).await,
        Some(repo.abs())
    );

    let fake_common = repo.join("fake-common");
    fs::rename(repo.join(".git"), &fake_common).unwrap();
    fs::create_dir(repo.join(".git")).unwrap();
    fs::write(
        checkout.join(".git"),
        format!(
            "gitdir: {}\n",
            fake_common.join("worktrees/feature-x").display()
        ),
    )
    .unwrap();
    assert_eq!(
        resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &checkout.abs()).await,
        None
    );
}

#[cfg(unix)]
#[tokio::test]
async fn resolve_root_git_project_for_trust_rejects_dot_git_swapped_after_stat() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let checkout = tmp.path().join("checkout");
    let attacker = tmp.path().join("attacker");
    write_linked_worktree_metadata(&repo, &checkout);
    fs::create_dir_all(&attacker).unwrap();
    fs::copy(checkout.join(".git"), attacker.join(".git")).unwrap();
    let fs = MetadataOverrideFileSystem {
        path: Some(PathUri::from_abs_path(&attacker.join(".git").abs())),
        replacement: Some(checkout.join(".git")),
        ..Default::default()
    };
    assert_eq!(
        codex_git_utils::resolve_root_git_project_for_trust(&fs, &attacker.abs()).await,
        None
    );
    assert!(
        std::fs::symlink_metadata(attacker.join(".git"))
            .unwrap()
            .is_symlink()
    );
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_preserves_canonical_checkout_case() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let checkout = tmp.path().join("real");
    let attacker = tmp.path().join("forged");
    write_linked_worktree_metadata(&repo, &checkout);
    fs::create_dir(&attacker).unwrap();
    fs::copy(checkout.join(".git"), attacker.join(".git")).unwrap();
    let registered_uri = PathUri::from_abs_path(&checkout.abs());
    let attacker_uri = PathUri::from_abs_path(&attacker.abs());
    // Simulate a case-sensitive Windows executor on every test host. General
    // PathUri equality treats these distinct filesystem identities as equal.
    for (canonical_attacker, expected) in [
        ("file:///C:/real", Some(repo.abs())),
        ("file:///C:/REAL", None),
    ] {
        let fs = MetadataOverrideFileSystem {
            canonical_overrides: vec![
                (
                    registered_uri.clone(),
                    PathUri::parse("file:///C:/real").unwrap(),
                ),
                (
                    attacker_uri.clone(),
                    PathUri::parse(canonical_attacker).unwrap(),
                ),
            ],
            ..Default::default()
        };
        assert_eq!(
            resolve_root_git_project_for_trust(&fs, &attacker.abs()).await,
            expected,
            "{canonical_attacker}"
        );
    }
}

// APFS rejects invalid UTF-8 filenames; exercise actual Git metadata on Linux.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn resolve_root_git_project_for_trust_preserves_non_utf8_worktree_paths() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let tmp = TempDir::new().unwrap();
    let original_repo = create_test_git_repo(&tmp).await;
    let repo = tmp.path().join(OsStr::from_bytes(b"repo-\xfe"));
    fs::rename(original_repo, &repo).unwrap();
    let common = repo.join(OsStr::from_bytes(b"git-data-\xfd"));
    let output = Command::new("git")
        .arg("init")
        .arg("--separate-git-dir")
        .arg(&common)
        .current_dir(&repo)
        .output()
        .await
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let checkout = tmp.path().join("checkout");
    let moved = tmp.path().join(OsStr::from_bytes(b"checkout-\xff"));
    let output = Command::new("git")
        .args(["worktree", "add", "--detach"])
        .arg(&checkout)
        .current_dir(&repo)
        .output()
        .await
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let output = Command::new("git")
        .args(["worktree", "move"])
        .arg(&checkout)
        .arg(&moved)
        .current_dir(&repo)
        .output()
        .await
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &moved.abs())
            .await
            .map(|path| normalize_for_path_comparison(path.as_path()).unwrap()),
        Some(normalize_for_path_comparison(&repo).unwrap())
    );
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_rejects_incomplete_metadata() {
    for filename in ["gitdir", "commondir"] {
        for contents in [None, Some(String::new()), Some("x".repeat(65_537))] {
            let tmp = TempDir::new().unwrap();
            let repo = tmp.path().join("repo");
            let checkout = tmp.path().join("checkout");
            let admin = write_linked_worktree_metadata(&repo, &checkout);
            let path = admin.join(filename);
            match contents {
                Some(contents) => fs::write(path, contents).unwrap(),
                None => fs::remove_file(path).unwrap(),
            }
            assert_eq!(
                resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &checkout.abs()).await,
                None,
                "{filename}"
            );
        }
    }
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_preserves_separate_git_directory() {
    let tmp = TempDir::new().unwrap();
    let repo = create_test_git_repo(&tmp).await;
    let common = repo.join(".git-data");
    let output = Command::new("git")
        .arg("init")
        .arg("--separate-git-dir")
        .arg(&common)
        .current_dir(&repo)
        .output()
        .await
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let checkout = tmp.path().join("checkout");
    let output = Command::new("git")
        .args(["worktree", "add", "--detach"])
        .arg(&checkout)
        .current_dir(&repo)
        .output()
        .await
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &checkout.abs())
            .await
            .map(|path| normalize_for_path_comparison(path.as_path()).unwrap()),
        Some(normalize_for_path_comparison(&repo).unwrap())
    );
}

#[tokio::test]
async fn resolve_root_git_project_for_trust_requires_repair_after_move() {
    let tmp = TempDir::new().unwrap();
    let repo = create_test_git_repo(&tmp).await;
    let checkout = tmp.path().join("checkout");
    let moved = tmp.path().join("moved");
    let output = Command::new("git")
        .args(["worktree", "add", "--detach"])
        .arg(&checkout)
        .current_dir(&repo)
        .output()
        .await
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    fs::rename(&checkout, &moved).unwrap();
    assert_eq!(
        resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &moved.abs()).await,
        None
    );
    let output = Command::new("git")
        .args(["worktree", "repair"])
        .arg(&moved)
        .current_dir(&repo)
        .output()
        .await
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &moved.abs())
            .await
            .map(|path| normalize_for_path_comparison(path.as_path()).unwrap()),
        Some(normalize_for_path_comparison(&repo).unwrap())
    );
}

#[cfg(unix)]
#[tokio::test]
async fn resolve_root_git_project_for_trust_preserves_checkout_directory_alias() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    let checkout = tmp.path().join("checkout");
    let alias = tmp.path().join("alias");
    write_linked_worktree_metadata(&repo, &checkout);
    std::os::unix::fs::symlink(&checkout, &alias).unwrap();
    assert_eq!(
        resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &alias.abs()).await,
        Some(repo.abs())
    );
}

#[cfg(unix)]
#[tokio::test]
async fn resolve_root_git_project_for_trust_rejects_symlinked_metadata() {
    for filename in ["gitdir", "commondir"] {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        let checkout = tmp.path().join("checkout");
        let admin = write_linked_worktree_metadata(&repo, &checkout);
        let path = admin.join(filename);
        let replacement = tmp.path().join("replacement");
        fs::rename(&path, &replacement).unwrap();
        std::os::unix::fs::symlink(replacement, path).unwrap();
        assert_eq!(
            resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &checkout.abs()).await,
            None
        );
    }
}
