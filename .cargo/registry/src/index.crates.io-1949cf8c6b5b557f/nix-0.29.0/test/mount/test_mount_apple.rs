use nix::errno::Errno;
use nix::mount::{mount, MntFlags};

#[test]
fn test_mount() {
    let res = mount::<str, str, str>("", "", MntFlags::empty(), None);
    assert_eq!(res, Err(Errno::ENOENT));
}
