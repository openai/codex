use super::*;
use pretty_assertions::assert_eq;
use test_case::test_case;

// Most cases have no independently masked subtree.
fn check_mounts(directory: &Path, device: &str, mountinfo: &[u8]) -> io::Result<()> {
    super::check_mounts(directory, device, mountinfo, /*masked_root*/ None)
}

#[test_case("/tmp", "/host-tmp", false; "ancestor alias")]
#[test_case("/tmp/codex-daemon-1000", "/alias", false; "directory alias")]
#[test_case("/tmp/codex-daemon-1000/rpc.sock", "/alias.sock", false; "socket alias")]
#[test_case("/", "/host", false; "root alias")]
#[test_case("/workspace", "/project", true; "unrelated project bind")]
#[test_case("/tmp", "/tmp", true; "same location")]
#[test_case("/tmp", "/host\\040tmp", false; "escaped alias")]
#[test_case("/other", "/tmp/codex-daemon-1000/nested", false; "nested mount")]
fn rejects_only_mounts_that_compromise_the_directory(root: &str, destination: &str, allowed: bool) {
    let mounts =
        format!("1 0 0:1 / / rw - ext4 disk rw\n2 1 0:1 {root} {destination} rw - ext4 disk rw\n");
    assert_eq!(
        check_mounts(
            Path::new("/tmp/codex-daemon-1000"),
            "0:1",
            mounts.as_bytes()
        )
        .is_ok(),
        allowed
    );
}

#[test_case("0:2", "mnt:[4026532835]", "/run/snapd/ns/example.mnt", Ok(()); "unrelated mount namespace")]
#[test_case("0:2", "net:[4026531840]", "/run/netns/example", Ok(()); "unrelated network namespace")]
#[test_case("0:2", "mnt:[4026532835]", "/tmp/codex-daemon-1000/ns", Err(io::ErrorKind::PermissionDenied); "nested namespace mount")]
#[test_case("0:1", "mnt:[4026532835]", "/run/snapd/ns/example.mnt", Err(io::ErrorKind::Other); "non-path root on socket filesystem")]
#[test_case("0:2", "mnt:[4026532835]", "relative/ns", Err(io::ErrorKind::Other); "relative destination")]
#[test_case("0:2", "mnt:[4026532835]", "/run/snapd/ns/\\invalid", Err(io::ErrorKind::Other); "invalid destination escape")]
fn validates_namespace_mounts_by_device_and_destination(
    device: &str,
    root: &str,
    destination: &str,
    expected: Result<(), io::ErrorKind>,
) {
    let mounts = format!(
        "1 0 0:1 / / rw - ext4 disk rw\n2 1 {device} {root} {destination} rw - nsfs nsfs rw\n"
    );
    assert_eq!(
        check_mounts(
            Path::new("/tmp/codex-daemon-1000"),
            "0:1",
            mounts.as_bytes()
        )
        .map_err(|error| error.kind()),
        expected
    );
}

#[test]
fn unrelated_namespace_mount_does_not_hide_a_socket_alias() {
    let mounts = b"1 0 0:1 / / rw - ext4 disk rw\n\
                   2 1 0:2 net:[4026531840] /run/netns/example rw - nsfs nsfs rw\n\
                   3 1 0:1 /tmp/codex-daemon-1000 /alias rw - ext4 disk rw\n";
    assert_eq!(
        check_mounts(Path::new("/tmp/codex-daemon-1000"), "0:1", mounts)
            .map_err(|error| error.kind()),
        Err(io::ErrorKind::PermissionDenied)
    );
}

#[test]
fn rejects_alias_when_tmp_is_itself_a_bind_mount() {
    let mounts = b"1 0 0:1 / / rw - ext4 disk rw\n2 1 0:1 /backing/tmp /tmp rw - ext4 disk rw\n";
    assert!(check_mounts(Path::new("/tmp/codex-daemon-1000"), "0:1", mounts).is_err());
    // A hidden deeper mount must not override the actual /tmp backing location.
    let hidden = [
        mounts.as_slice(),
        b"3 1 0:1 /tmp/codex-daemon-1000 /tmp/codex-daemon-1000 rw - ext4 disk rw\n",
    ]
    .concat();
    assert!(check_mounts(Path::new("/tmp/codex-daemon-1000"), "0:1", &hidden).is_err());
}

#[test]
fn accepts_private_tmp_filesystem_but_rejects_ambiguous_stacked_mounts() {
    let mounts = "1 0 0:1 / / rw - ext4 disk rw\n2 1 0:2 / /tmp rw - tmpfs tmpfs rw\n";
    let directory = Path::new("/tmp/codex-daemon-1000");
    assert!(check_mounts(directory, "0:2", mounts.as_bytes()).is_ok());
    let stacked = format!("{mounts}3 2 0:2 /other /tmp rw - tmpfs tmpfs rw\n");
    assert!(check_mounts(directory, "0:2", stacked.as_bytes()).is_err());
}

#[test]
fn masked_wslg_alias_does_not_allow_other_exposed_aliases() {
    let mounts = "1 0 0:1 / / rw - ext4 disk rw\n2 1 0:1 / /mnt/wslg/distro rw - ext4 disk rw\n";
    let directory = Path::new("/tmp/codex-daemon-1000");
    let mask = Some(Path::new(crate::bwrap::WSLG_DISTRO_ROOT));
    assert!(check_mounts(directory, "0:1", mounts.as_bytes()).is_err());
    assert!(super::check_mounts(directory, "0:1", mounts.as_bytes(), mask).is_ok());
    let exposed = format!("{mounts}3 1 0:1 /tmp /host-tmp rw - ext4 disk rw\n");
    assert!(super::check_mounts(directory, "0:1", exposed.as_bytes(), mask).is_err());
}
