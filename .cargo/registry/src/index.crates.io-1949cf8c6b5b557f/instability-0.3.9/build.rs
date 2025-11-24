fn main() {
    println!("cargo:rustc-check-cfg=cfg(instability_disable_unstable_docs)");
}
