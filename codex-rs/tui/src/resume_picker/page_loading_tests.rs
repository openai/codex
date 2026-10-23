//! Checks that a cursor keeps its original directory scope until the listing restarts.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn directory_scope_changes_only_affect_the_next_listing_cycle() {
    let root = tempfile::tempdir().expect("temporary repository");
    let primary = root.path().join("primary");
    let linked = root.path().join("linked");
    let admin = primary.join(".git/worktrees/linked");
    std::fs::create_dir_all(primary.join(".git")).expect("primary git directory");
    std::fs::write(primary.join(".git/HEAD"), "ref: refs/heads/main\n").expect("primary HEAD");
    let primary = dunce::canonicalize(primary).expect("canonical primary");
    let single = Some(ThreadListCwdFilter::One(
        primary.to_string_lossy().into_owned(),
    ));
    let mut filter = PageCwdFilter::default();
    assert_eq!(
        filter.for_request(
            /*cursor*/ None,
            Some(&primary),
            SessionFilterMode::Cwd,
            /*uses_remote_filesystem*/ false,
        ),
        single
    );

    std::fs::create_dir_all(&admin).expect("linked worktree registration");
    std::fs::create_dir_all(&linked).expect("linked checkout");
    std::fs::write(admin.join("commondir"), "../..\n").expect("common directory");
    std::fs::write(
        admin.join("gitdir"),
        linked.join(".git").display().to_string(),
    )
    .expect("linked backlink");
    std::fs::write(linked.join(".git"), format!("gitdir: {}", admin.display()))
        .expect("linked git file");
    let linked = dunce::canonicalize(linked).expect("canonical linked checkout");
    let cursor = PageCursor::AppServer("next-page".to_string());
    assert_eq!(
        filter.for_request(
            Some(&cursor),
            Some(&primary),
            SessionFilterMode::Repo,
            /*uses_remote_filesystem*/ false,
        ),
        single
    );
    assert_eq!(
        filter.for_request(
            /*cursor*/ None,
            Some(&primary),
            SessionFilterMode::Cwd,
            /*uses_remote_filesystem*/ false,
        ),
        single
    );
    let repo = Some(ThreadListCwdFilter::Many(vec![
        primary.to_string_lossy().into_owned(),
        linked.to_string_lossy().into_owned(),
    ]));
    assert_eq!(
        filter.for_request(
            /*cursor*/ None,
            Some(&primary),
            SessionFilterMode::Repo,
            /*uses_remote_filesystem*/ false,
        ),
        repo
    );
    std::fs::remove_file(linked.join(".git")).expect("remove linked checkout registration");
    assert_eq!(
        filter.for_request(
            Some(&cursor),
            Some(&primary),
            SessionFilterMode::Cwd,
            /*uses_remote_filesystem*/ false,
        ),
        repo
    );
    assert_eq!(
        filter.for_request(
            /*cursor*/ None,
            Some(&primary),
            SessionFilterMode::Repo,
            /*uses_remote_filesystem*/ false,
        ),
        single
    );
    assert_eq!(
        filter.for_request(
            /*cursor*/ None,
            Some(&primary),
            SessionFilterMode::All,
            /*uses_remote_filesystem*/ false,
        ),
        None
    );
}
