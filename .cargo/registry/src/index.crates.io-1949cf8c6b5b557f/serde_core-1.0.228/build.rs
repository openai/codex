use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::str;

const PRIVATE: &str = "\
#[doc(hidden)]
pub mod __private$$ {
    #[doc(hidden)]
    pub use crate::private::*;
}
";

// The rustc-cfg strings below are *not* public API. Please let us know by
// opening a GitHub issue if your build environment requires some way to enable
// these cfgs other than by executing our build script.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let patch_version = env::var("CARGO_PKG_VERSION_PATCH").unwrap();
    let module = PRIVATE.replace("$$", &patch_version);
    fs::write(out_dir.join("private.rs"), module).unwrap();

    let minor = match rustc_minor_version() {
        Some(minor) => minor,
        None => return,
    };

    if minor >= 77 {
        println!("cargo:rustc-check-cfg=cfg(if_docsrs_then_no_serde_core)");
        println!("cargo:rustc-check-cfg=cfg(no_core_cstr)");
        println!("cargo:rustc-check-cfg=cfg(no_core_error)");
        println!("cargo:rustc-check-cfg=cfg(no_core_net)");
        println!("cargo:rustc-check-cfg=cfg(no_core_num_saturating)");
        println!("cargo:rustc-check-cfg=cfg(no_diagnostic_namespace)");
        println!("cargo:rustc-check-cfg=cfg(no_serde_derive)");
        println!("cargo:rustc-check-cfg=cfg(no_std_atomic)");
        println!("cargo:rustc-check-cfg=cfg(no_std_atomic64)");
        println!("cargo:rustc-check-cfg=cfg(no_target_has_atomic)");
    }

    let target = env::var("TARGET").unwrap();
    let emscripten = target == "asmjs-unknown-emscripten" || target == "wasm32-unknown-emscripten";

    // Support for #[cfg(target_has_atomic = "...")] stabilized in Rust 1.60.
    if minor < 60 {
        println!("cargo:rustc-cfg=no_target_has_atomic");
        // Allowlist of archs that support std::sync::atomic module. This is
        // based on rustc's compiler/rustc_target/src/spec/*.rs.
        let has_atomic64 = target.starts_with("x86_64")
            || target.starts_with("i686")
            || target.starts_with("aarch64")
            || target.starts_with("powerpc64")
            || target.starts_with("sparc64")
            || target.starts_with("mips64el")
            || target.starts_with("riscv64");
        let has_atomic32 = has_atomic64 || emscripten;
        if minor < 34 || !has_atomic64 {
            println!("cargo:rustc-cfg=no_std_atomic64");
        }
        if minor < 34 || !has_atomic32 {
            println!("cargo:rustc-cfg=no_std_atomic");
        }
    }

    // Current minimum supported version of serde_derive crate is Rust 1.61.
    if minor < 61 {
        println!("cargo:rustc-cfg=no_serde_derive");
    }

    // Support for core::ffi::CStr and alloc::ffi::CString stabilized in Rust 1.64.
    // https://blog.rust-lang.org/2022/09/22/Rust-1.64.0.html#c-compatible-ffi-types-in-core-and-alloc
    if minor < 64 {
        println!("cargo:rustc-cfg=no_core_cstr");
    }

    // Support for core::num::Saturating and std::num::Saturating stabilized in Rust 1.74
    // https://blog.rust-lang.org/2023/11/16/Rust-1.74.0.html#stabilized-apis
    if minor < 74 {
        println!("cargo:rustc-cfg=no_core_num_saturating");
    }

    // Support for core::net stabilized in Rust 1.77.
    // https://blog.rust-lang.org/2024/03/21/Rust-1.77.0.html
    if minor < 77 {
        println!("cargo:rustc-cfg=no_core_net");
    }

    // Support for the `#[diagnostic]` tool attribute namespace
    // https://blog.rust-lang.org/2024/05/02/Rust-1.78.0.html#diagnostic-attributes
    if minor < 78 {
        println!("cargo:rustc-cfg=no_diagnostic_namespace");
    }

    // The Error trait became available in core in 1.81.
    // https://blog.rust-lang.org/2024/09/05/Rust-1.81.0.html#coreerrorerror
    if minor < 81 {
        println!("cargo:rustc-cfg=no_core_error");
    }
}

fn rustc_minor_version() -> Option<u32> {
    let rustc = env::var_os("RUSTC")?;
    let output = Command::new(rustc).arg("--version").output().ok()?;
    let version = str::from_utf8(&output.stdout).ok()?;
    let mut pieces = version.split('.');
    if pieces.next() != Some("rustc 1") {
        return None;
    }
    pieces.next()?.parse().ok()
}
