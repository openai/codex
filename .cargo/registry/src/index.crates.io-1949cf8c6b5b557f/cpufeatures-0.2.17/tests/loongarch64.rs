//! LoongArch64 tests

#![cfg(target_arch = "loongarch64")]

cpufeatures::new!(
    lacaps, "cpucfg", "lam", "ual", "fpu", "lsx", "lasx", "crc32", "complex", "crypto", "lvz",
    "lbt.x86", "lbt.arm", "lbt.mips", "ptw"
);

#[test]
fn init() {
    let token: lacaps::InitToken = lacaps::init();
    assert_eq!(token.get(), lacaps::get());
}

#[test]
fn init_get() {
    let (token, val) = lacaps::init_get();
    assert_eq!(val, token.get());
}
