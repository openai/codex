use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

use super::BrowserProfileName;
use super::BrowserProfileStore;
use crate::sandbox::BrowserLaunchContext;

#[test]
fn profile_names_reject_traversal_and_non_ascii_input() {
    for name in ["", ".hidden", "../other", "name/child", "café"] {
        assert!(
            BrowserProfileName::parse(name).is_err(),
            "accepted {name:?}"
        );
    }
    assert_eq!(
        BrowserProfileName::parse("work-login_2")
            .expect("valid profile")
            .as_str(),
        "work-login_2"
    );
}

#[test]
fn profiles_are_scoped_by_workspace_and_can_be_forgotten() {
    let codex_home = tempfile::tempdir().expect("codex home");
    let codex_home = AbsolutePathBuf::from_absolute_path(codex_home.path()).expect("absolute home");
    let workspace_root = codex_home.join("workspace");
    let context = BrowserLaunchContext {
        codex_home: Some(codex_home),
        workspace_root: Some(workspace_root),
        ..Default::default()
    };
    let store = BrowserProfileStore::from_context(&context)
        .expect("profile store")
        .expect("configured profile store");
    let profile = BrowserProfileName::parse("work").expect("profile name");

    let path = store.create(&profile).expect("create profile");
    assert!(path.as_path().is_dir());
    assert_eq!(store.list().expect("list profiles").profiles, vec!["work"]);
    store.forget(&profile).expect("forget profile");
    assert!(store.list().expect("list profiles").profiles.is_empty());
}

#[cfg(unix)]
#[test]
fn forgetting_a_locked_profile_is_refused() {
    let codex_home = tempfile::tempdir().expect("codex home");
    let codex_home = AbsolutePathBuf::from_absolute_path(codex_home.path()).expect("absolute home");
    let workspace_root = codex_home.join("workspace");
    let context = BrowserLaunchContext {
        codex_home: Some(codex_home),
        workspace_root: Some(workspace_root),
        ..Default::default()
    };
    let store = BrowserProfileStore::from_context(&context)
        .expect("profile store")
        .expect("configured profile store");
    let profile = BrowserProfileName::parse("work").expect("profile name");
    store.create(&profile).expect("create profile");

    let (_path, profile_lock) = store.lock_existing(&profile).expect("lock profile");
    let error = store
        .forget(&profile)
        .expect_err("locked profile must not be deleted");
    assert_eq!(
        error.to_string(),
        "browser profile `work` is already in use"
    );

    drop(profile_lock);
    store.forget(&profile).expect("forget unlocked profile");
}
