use std::env;

// Based on https://github.com/serde-rs/serde/blob/master/serde/build.rs

fn main() {
    let target = env::var("TARGET").unwrap();
    let emscripten = target == "asmjs-unknown-emscripten" || target == "wasm32-unknown-emscripten";

    // Whitelist of archs that support std::sync::atomic module. Ideally we
    // would use #[cfg(target_has_atomic = "...")] but it is not stable yet.
    // Instead this is based on rustc's src/librustc_target/spec/*.rs.
    let has_atomic64 = target.starts_with("x86_64")
        || target.starts_with("i686")
        || target.starts_with("aarch64")
        || target.starts_with("powerpc64")
        || target.starts_with("sparc64")
        || target.starts_with("mips64el");
    let has_atomic32 = has_atomic64 || emscripten;
    if has_atomic64 {
        println!("cargo:rustc-cfg=std_atomic64");
    }
    if has_atomic32 {
        println!("cargo:rustc-cfg=std_atomic");
    }
}
