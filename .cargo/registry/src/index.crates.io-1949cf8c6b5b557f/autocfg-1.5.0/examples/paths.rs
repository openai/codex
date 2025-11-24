extern crate autocfg;

fn main() {
    // Normally, cargo will set `OUT_DIR` for build scripts.
    let ac = autocfg::AutoCfg::with_dir("target").unwrap();

    // since ancient times...
    ac.emit_has_path("std::vec::Vec");
    ac.emit_path_cfg("std::vec::Vec", "has_vec");

    // rustc 1.10.0
    ac.emit_has_path("std::panic::PanicInfo");
    ac.emit_path_cfg("std::panic::PanicInfo", "has_panic_info");

    // rustc 1.20.0
    ac.emit_has_path("std::mem::ManuallyDrop");
    ac.emit_path_cfg("std::mem::ManuallyDrop", "has_manually_drop");

    // rustc 1.25.0
    ac.emit_has_path("std::ptr::NonNull");
    ac.emit_path_cfg("std::ptr::NonNull", "has_non_null");
}
