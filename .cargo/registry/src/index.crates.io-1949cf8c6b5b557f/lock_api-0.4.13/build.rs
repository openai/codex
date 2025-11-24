fn main() {
    let cfg = autocfg::new();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(has_const_fn_trait_bound)");
    if cfg.probe_rustc_version(1, 61) {
        println!("cargo:rustc-cfg=has_const_fn_trait_bound");
    }
}
