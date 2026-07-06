use pretty_assertions::assert_eq;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

use super::read_link_if_symlink;
#[cfg(target_os = "linux")]
use super::route_contains_process_relative_procfs_path;

#[test]
fn ordinary_paths_are_not_treated_as_failed_symlink_reads() {
    let fixture = tempfile::tempdir().expect("fixture");
    let directory = fixture.path().join("directory");
    let file = fixture.path().join("file");
    std::fs::create_dir(&directory).expect("directory");
    std::fs::write(&file, b"ordinary file").expect("file");

    assert_eq!(
        read_link_if_symlink(&directory).expect("ordinary directory"),
        None
    );
    assert_eq!(read_link_if_symlink(&file).expect("ordinary file"), None);
}

#[cfg(target_os = "linux")]
#[test]
fn process_relative_procfs_detection_follows_aliases_without_rejecting_ordinary_paths() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let external = fixture.path().join("external");
    let ordinary_alias = fixture.path().join("ordinary-alias");
    let procfs_alias = fixture.path().join("procfs-alias");
    let procfs_root_alias = fixture.path().join("procfs-root-alias");
    let chained_alias = fixture.path().join("chained-alias");
    let literal = fixture.path().join("proc/self/cwd/config");
    std::fs::create_dir_all(&external).expect("external directory");
    std::fs::create_dir_all(literal.parent().expect("literal parent"))
        .expect("literal directories");
    std::fs::write(&literal, "[safe]\n").expect("literal config");
    symlink(&external, &ordinary_alias).expect("ordinary alias");
    symlink("/proc/self/cwd", &procfs_alias).expect("procfs alias");
    symlink("/proc", &procfs_root_alias).expect("procfs root alias");
    symlink("procfs-alias", &chained_alias).expect("chained alias");

    let mut procfs_paths = vec![
        PathBuf::from("/proc/self/cwd/missing-config"),
        PathBuf::from("/proc/thread-self/cwd/missing-config"),
        PathBuf::from("/proc/self/fd/1048575/missing-config"),
        PathBuf::from("/proc/self/exe"),
        PathBuf::from(format!("/proc/{}/cwd/missing-config", std::process::id())),
        PathBuf::from("/proc/4294967295/cwd/missing-config"),
        procfs_alias.join("missing-config"),
        procfs_root_alias.join("4294967295/cwd/missing-config"),
        chained_alias.join("missing-config"),
    ];
    if Path::new("/dev/fd").exists() {
        procfs_paths.push(PathBuf::from("/dev/fd/1048575/missing-config"));
    }
    for path in procfs_paths {
        assert!(
            route_contains_process_relative_procfs_path(&path).expect("inspect procfs route"),
            "expected process-relative procfs route in {}",
            path.display()
        );
    }

    for path in [
        PathBuf::from("/proc/version"),
        ordinary_alias.join("missing-config"),
        literal,
    ] {
        assert!(
            !route_contains_process_relative_procfs_path(&path).expect("inspect ordinary route"),
            "unexpected process-relative procfs route in {}",
            path.display()
        );
    }
}
