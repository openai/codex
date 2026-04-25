fn main() {
    println!("cargo:rerun-if-changed=src/platform/macos_provider.h");
    println!("cargo:rerun-if-changed=src/platform/macos_provider.m");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    cc::Build::new()
        .file("src/platform/macos_provider.m")
        .flag("-fobjc-arc")
        .compile("codex_device_key_macos_provider");

    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=Security");
    println!("cargo:rustc-link-lib=framework=LocalAuthentication");
}
