// Automatically detect tsan in a way that's compatible with both stable (which
// doesn't support sanitizers) and nightly (which does). Works because build
// scripts gets `cfg` info, even if the cfg is unstable.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(tsan_enabled)");
    let santizer_list = std::env::var("CARGO_CFG_SANITIZE").unwrap_or_default();
    if santizer_list.contains("thread") {
        println!("cargo:rustc-cfg=tsan_enabled");
    }
}
