extern crate autocfg;

fn main() {
    // Normally, cargo will set `OUT_DIR` for build scripts.
    let ac = autocfg::AutoCfg::with_dir("target").unwrap();

    // since ancient times...
    ac.emit_has_trait("std::ops::Add");
    ac.emit_trait_cfg("std::ops::Add", "has_ops");

    // trait parameters have to be provided
    ac.emit_has_trait("std::borrow::Borrow<str>");
    ac.emit_trait_cfg("std::borrow::Borrow<str>", "has_borrow");

    // rustc 1.8.0
    ac.emit_has_trait("std::ops::AddAssign");
    ac.emit_trait_cfg("std::ops::AddAssign", "has_assign_ops");

    // rustc 1.12.0
    ac.emit_has_trait("std::iter::Sum");
    ac.emit_trait_cfg("std::iter::Sum", "has_sum");

    // rustc 1.28.0
    ac.emit_has_trait("std::alloc::GlobalAlloc");
    ac.emit_trait_cfg("std::alloc::GlobalAlloc", "has_global_alloc");
}
