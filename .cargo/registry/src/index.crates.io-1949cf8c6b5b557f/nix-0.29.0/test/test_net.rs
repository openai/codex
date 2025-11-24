use nix::net::if_::*;

#[cfg(linux_android)]
const LOOPBACK: &[u8] = b"lo";

#[cfg(not(any(linux_android, target_os = "haiku")))]
const LOOPBACK: &[u8] = b"lo0";

#[cfg(target_os = "haiku")]
const LOOPBACK: &[u8] = b"loop";

#[test]
fn test_if_nametoindex() {
    if_nametoindex(LOOPBACK).expect("assertion failed");
}

#[test]
fn test_if_indextoname() {
    let loopback_index = if_nametoindex(LOOPBACK).expect("assertion failed");
    assert_eq!(
        if_indextoname(loopback_index)
            .expect("assertion failed")
            .as_bytes(),
        LOOPBACK
    );
}
