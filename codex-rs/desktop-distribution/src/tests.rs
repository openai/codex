use std::fs;

use tempfile::tempdir;

use super::DesktopDistributionError;
use super::ResourceKind;
use super::canonical_absolute;
use super::contained_existing_path;
use super::current_or_installed;

#[test]
fn locator_discovers_an_install_when_current_process_is_unrelated() {
    let located: Result<&str, &str> =
        current_or_installed(|| Ok(None), || Ok("authenticated installed distribution"));

    assert_eq!(located, Ok("authenticated installed distribution"));
}

#[test]
fn locator_does_not_fallback_after_current_distribution_failure() {
    let located: Result<&str, &str> = current_or_installed(
        || Err("current distribution failed authentication"),
        || panic!("authentication failures must not fall back"),
    );

    assert_eq!(located, Err("current distribution failed authentication"));
}

#[test]
fn contained_resources_reject_parent_traversal() {
    let temp = tempdir().expect("tempdir");
    let resources = temp.path().join("resources");
    fs::create_dir_all(&resources).expect("resources");
    let resources = canonical_absolute(&resources, "test").expect("canonical resources");

    let error = contained_existing_path(
        &resources,
        std::path::Path::new("../outside"),
        ResourceKind::File,
    )
    .expect_err("parent traversal must fail");

    assert!(matches!(
        error,
        DesktopDistributionError::Containment { .. }
    ));
}

#[cfg(unix)]
#[test]
fn contained_resources_reject_symlink_escape() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().expect("tempdir");
    let resources = temp.path().join("resources");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&resources).expect("resources");
    fs::write(&outside, "outside").expect("outside file");
    symlink(&outside, resources.join("hook.sh")).expect("symlink");
    let resources = canonical_absolute(&resources, "test").expect("canonical resources");

    let error = contained_existing_path(
        &resources,
        std::path::Path::new("hook.sh"),
        ResourceKind::File,
    )
    .expect_err("symlink must fail");

    assert!(matches!(
        error,
        DesktopDistributionError::Containment { .. }
    ));
}

#[cfg(unix)]
#[test]
fn contained_resources_reject_symlink_components_even_inside_root() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().expect("tempdir");
    let resources = temp.path().join("resources");
    let real_hooks = resources.join("real-hooks");
    fs::create_dir_all(&real_hooks).expect("real hooks");
    fs::write(real_hooks.join("hooks.json"), "{}").expect("hook declaration");
    symlink("real-hooks", resources.join("hooks")).expect("symlink");
    let resources = canonical_absolute(&resources, "test").expect("canonical resources");

    let error = contained_existing_path(
        &resources,
        std::path::Path::new("hooks/hooks.json"),
        ResourceKind::File,
    )
    .expect_err("symlink components must fail");

    assert!(matches!(
        error,
        DesktopDistributionError::Containment { .. }
    ));
}

#[test]
fn contained_resources_return_canonical_files() {
    let temp = tempdir().expect("tempdir");
    let resources = temp.path().join("resources");
    let hook = resources.join("plugins/demo/hooks/hooks.json");
    fs::create_dir_all(hook.parent().expect("hook parent")).expect("hook parent");
    fs::write(&hook, "{}").expect("hook");
    let resources = canonical_absolute(&resources, "test").expect("canonical resources");

    let resolved = contained_existing_path(
        &resources,
        std::path::Path::new("plugins/demo/hooks/hooks.json"),
        ResourceKind::File,
    )
    .expect("contained file");

    assert_eq!(
        resolved,
        canonical_absolute(&hook, "test").expect("canonical hook")
    );
}
