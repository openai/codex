fn main() {
    if version_check::is_min_version("1.41.0").unwrap_or(false) {
        println!("cargo:rustc-cfg=relaxed_coherence");
    }
}
