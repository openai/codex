use super::is_duplicate_root;
use super::mount_identity;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;

#[test]
fn ordinary_paths_do_not_identify_a_duplicate_root() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let file = temp.path().join("file");
    fs::write(&file, "fixture").expect("write file");
    let link = temp.path().join("link");
    std::os::unix::fs::symlink("/", &link).expect("create symlink");
    let missing = temp.path().join("missing");
    let not_a_directory = file.join("child");
    for path in [temp.path(), &file, &link, &missing, &not_a_directory] {
        assert!(!is_duplicate_root(path).expect("inspect path"));
    }
    assert!(is_duplicate_root(Path::new("/")).expect("inspect root"));
}

#[test]
fn mount_identity_distinguishes_roots_on_the_same_device() {
    let mounts = "36 25 8:16 / / rw - ext4 /dev/sdb rw\n\
                  39 25 8:16 / /mnt/wslg/distro rw - ext4 /dev/sdb rw\n\
                  40 25 8:16 /other /unrelated rw - ext4 /dev/sdb rw\n\
                  41 25 0:45 / /different-device rw - tmpfs tmpfs rw\n";
    assert_eq!(
        mount_identity(mounts, "/").unwrap(),
        mount_identity(mounts, "/mnt/wslg/distro").unwrap()
    );
    assert_ne!(
        mount_identity(mounts, "/").unwrap(),
        mount_identity(mounts, "/unrelated").unwrap()
    );
    assert_ne!(
        mount_identity(mounts, "/").unwrap(),
        mount_identity(mounts, "/different-device").unwrap()
    );
    assert_eq!(mount_identity(mounts, "/missing").unwrap(), None);
}

#[test]
fn conflicting_stacked_mounts_are_ambiguous() {
    let mounts = "36 25 8:16 / / rw - ext4 /dev/sdb rw\n\
                  39 25 8:16 / /mnt/wslg/distro rw - ext4 /dev/sdb rw\n\
                  40 39 0:45 / /mnt/wslg/distro rw - tmpfs tmpfs rw\n";
    assert!(mount_identity(mounts, "/mnt/wslg/distro").is_err());
}
