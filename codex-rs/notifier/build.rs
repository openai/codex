fn main() {
    #[cfg(target_os = "macos")]
    {
        use std::path::PathBuf;

        let source = PathBuf::from("src/macos/notification.mm");
        println!("cargo:rerun-if-changed={}", source.display());

        let mut build = cc::Build::new();
        build
            .cpp(true)
            .flag("-fobjc-arc")
            .flag("-fmodules")
            .file(&source);
        build.compile("codex_macos_notification");

        println!("cargo:rustc-link-lib=framework=UserNotifications");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
}
