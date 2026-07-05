use std::collections::HashMap;

use codex_protocol::permissions::FileSystemAccessMode;
use pretty_assertions::assert_eq;

use super::browser_file_system_policy;
use super::ensure_isolated_binary_root;
use super::prepare_browser_launch;
use crate::network::BrowserNetworkPolicy;
use crate::sandbox::BrowserLaunchContext;
use codex_utils_absolute_path::AbsolutePathBuf;

#[test]
fn browser_policy_can_write_only_its_runtime_root() {
    let root = tempfile::tempdir().expect("test root");
    let root = AbsolutePathBuf::from_absolute_path(root.path()).expect("absolute test root");
    let browser_root = root.join("browser-runtime");
    let binary_root = root.join("carbonyl-bundle");
    let workspace_root = root.join("workspace");
    let policy = browser_file_system_policy(
        &browser_root,
        &browser_root.join("profile"),
        &binary_root,
        &HashMap::new(),
    )
    .expect("browser policy");

    assert_eq!(
        policy.resolve_access_with_cwd(browser_root.as_path(), browser_root.as_path()),
        FileSystemAccessMode::Write
    );
    assert_eq!(
        policy.resolve_access_with_cwd(binary_root.as_path(), browser_root.as_path()),
        FileSystemAccessMode::Read
    );
    assert_eq!(
        policy.resolve_access_with_cwd(
            browser_root.join("workspace").as_path(),
            browser_root.as_path()
        ),
        FileSystemAccessMode::Write
    );
    assert_eq!(
        policy.resolve_access_with_cwd(workspace_root.as_path(), browser_root.as_path()),
        FileSystemAccessMode::Deny
    );
    #[cfg(target_os = "macos")]
    {
        let readable_system_resources = [
            "/System/Library/CoreServices/SystemAppearance.bundle/Contents/Resources/SystemAppearance.car",
            "/System/Library/CoreServices/SystemVersion.bundle/Contents/Resources/English.lproj/SystemVersion.strings",
            "/System/Library/Fonts/LastResort.otf",
        ];
        let unrelated_core_service = AbsolutePathBuf::from_absolute_path(
            "/System/Library/CoreServices/Finder.app/Contents/MacOS/Finder",
        )
        .expect("absolute unrelated CoreServices path");

        for path in readable_system_resources {
            let path = AbsolutePathBuf::from_absolute_path(path).expect("absolute system path");
            assert_eq!(
                policy.resolve_access_with_cwd(path.as_path(), browser_root.as_path()),
                FileSystemAccessMode::Read
            );
        }
        assert_eq!(
            policy
                .resolve_access_with_cwd(unrelated_core_service.as_path(), browser_root.as_path()),
            FileSystemAccessMode::Deny
        );
    }
}

#[test]
fn managed_proxy_launches_fail_closed() {
    let root = tempfile::tempdir().expect("test root");
    let root = AbsolutePathBuf::from_absolute_path(root.path()).expect("absolute test root");
    let Err(error) = prepare_browser_launch(
        root.join("carbonyl").as_path(),
        Vec::new(),
        &root.join("runtime"),
        &root.join("runtime/profile"),
        HashMap::new(),
        &BrowserNetworkPolicy::ManagedProxy {
            http_addr: "127.0.0.1:8080".parse().expect("proxy address"),
        },
        &BrowserLaunchContext::default(),
    ) else {
        panic!("managed proxy must be rejected");
    };

    assert!(
        error
            .to_string()
            .contains("without bypassing the managed proxy")
    );
}

#[test]
fn binary_bundle_must_not_contain_the_workspace() {
    let root = tempfile::tempdir().expect("test root");
    let root = AbsolutePathBuf::from_absolute_path(root.path()).expect("absolute test root");
    let bundle = root.join("bundle");
    let workspace = bundle.join("workspace");
    let context = BrowserLaunchContext {
        workspace_root: Some(workspace),
        ..Default::default()
    };

    let error = ensure_isolated_binary_root(
        &bundle,
        &root.join("runtime"),
        &root.join("runtime/profile"),
        &context,
    )
    .expect_err("overlapping bundle must be rejected");

    assert!(error.to_string().contains("dedicated bundle"));
}
