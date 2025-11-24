fn main() {
    // If "std" is explicitly requested, don't bother probing the target for it.
    match std::env::var_os("CARGO_FEATURE_STD") {
        Some(_) => autocfg::emit("has_std"),
        None => autocfg::new().emit_sysroot_crate("std"),
    }
    autocfg::rerun_path("build.rs");
}
