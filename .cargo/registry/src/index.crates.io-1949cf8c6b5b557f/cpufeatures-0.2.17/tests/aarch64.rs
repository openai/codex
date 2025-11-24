//! ARM64 tests

#![cfg(target_arch = "aarch64")]

cpufeatures::new!(armcaps, "aes", "sha2", "sha3", "sm4");

#[test]
fn init() {
    let token: armcaps::InitToken = armcaps::init();
    assert_eq!(token.get(), armcaps::get());
}

#[test]
fn init_get() {
    let (token, val) = armcaps::init_get();
    assert_eq!(val, token.get());
}
