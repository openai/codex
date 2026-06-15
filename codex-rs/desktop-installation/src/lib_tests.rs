use pretty_assertions::assert_eq;
use std::fs;

use super::DesktopInstallationError;
use super::DesktopResources;
#[cfg(unix)]
use super::VerifiedDesktopInstallation;
use super::canonical;

#[test]
fn resolves_strictly_contained_files_and_directories() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("resources");
    let directory = root.join("plugins/demo");
    fs::create_dir_all(&directory).expect("create directory");
    let file = directory.join("hook.json");
    fs::write(&file, "{}").expect("write file");
    let resources = DesktopResources::from_trusted_path(root).expect("Desktop resources");

    assert_eq!(
        resources
            .contained_directory("plugins/demo")
            .expect("contained directory"),
        canonical(&directory, "test directory").expect("canonical directory")
    );
    assert_eq!(
        resources
            .contained_file("plugins/demo/hook.json")
            .expect("contained file"),
        canonical(&file, "test file").expect("canonical file")
    );
}

#[test]
fn rejects_non_normal_and_wrong_kind_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("resources");
    fs::create_dir_all(root.join("plugins")).expect("create directory");
    let resources = DesktopResources::from_trusted_path(root).expect("Desktop resources");

    for path in ["", ".", "../resources/plugins", "/tmp"] {
        assert!(matches!(
            resources.contained_directory(path),
            Err(DesktopInstallationError::Containment(_))
        ));
    }
    assert!(matches!(
        resources.contained_file("plugins"),
        Err(DesktopInstallationError::Containment(_))
    ));
}

#[cfg(unix)]
#[test]
fn rejects_symlink_traversal() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("resources");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&root).expect("create root");
    fs::create_dir_all(&outside).expect("create outside");
    fs::write(outside.join("hook.json"), "{}").expect("write file");
    symlink(&outside, root.join("plugins")).expect("create symlink");
    let resources = DesktopResources::from_trusted_path(root).expect("Desktop resources");

    assert!(matches!(
        resources.contained_file("plugins/hook.json"),
        Err(DesktopInstallationError::Containment(_))
    ));

    let app_root = temp.path().join("Codex.app");
    let resources_link = app_root.join("Contents/Resources");
    fs::create_dir_all(resources_link.parent().expect("resources parent"))
        .expect("create app contents");
    symlink(&outside, &resources_link).expect("create resources symlink");
    assert!(matches!(
        VerifiedDesktopInstallation::from_paths(app_root, resources_link),
        Err(DesktopInstallationError::Containment(_))
    ));
}
