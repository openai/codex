use nix::sys::sysinfo::*;

#[test]
fn sysinfo_works() {
    let info = sysinfo().unwrap();

    let (l1, l5, l15) = info.load_average();
    assert!(l1 >= 0.0);
    assert!(l5 >= 0.0);
    assert!(l15 >= 0.0);

    info.uptime(); // just test Duration construction

    assert!(
        info.swap_free() <= info.swap_total(),
        "more swap available than installed (free: {}, total: {})",
        info.swap_free(),
        info.swap_total()
    );
}
