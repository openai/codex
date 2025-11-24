use cfg_aliases::cfg_aliases;

#[test]
fn basic_setup() {
    // Same as in the docs.
    // Note that tests build this already, but unfortunately this doesn't catch clippy lints!
    // See https://github.com/rust-lang/rust/issues/56232
    cfg_aliases! {
        // Platforms
        wasm: { target_arch = "wasm32" },
        android: { target_os = "android" },
        macos: { target_os = "macos" },
        linux: { target_os = "linux" },
        // Backends
        surfman: { all(unix, feature = "surfman", not(wasm)) },
        glutin: { all(feature = "glutin", not(wasm)) },
        wgl: { all(windows, feature = "wgl", not(wasm)) },
        dummy: { not(any(wasm, glutin, wgl, surfman)) },
    };
}
