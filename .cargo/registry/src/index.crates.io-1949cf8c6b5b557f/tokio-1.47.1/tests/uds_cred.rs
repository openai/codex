#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(all(unix, not(target_os = "dragonfly"), not(miri)))] // No `getsockopt` on miri.

use tokio::net::UnixStream;

use libc::getegid;
use libc::geteuid;

#[tokio::test]
#[cfg_attr(
    target_os = "netbsd",
    ignore = "NetBSD does not support getpeereid() for sockets created by socketpair()"
)]
async fn test_socket_pair() {
    let (a, b) = UnixStream::pair().unwrap();
    let cred_a = a.peer_cred().unwrap();
    let cred_b = b.peer_cred().unwrap();
    assert_eq!(cred_a, cred_b);

    let uid = unsafe { geteuid() };
    let gid = unsafe { getegid() };

    assert_eq!(cred_a.uid(), uid);
    assert_eq!(cred_a.gid(), gid);
}
