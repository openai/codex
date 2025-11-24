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
use serde_core::__private$$ as serde_core_private;
";

// The rustc-cfg strings below are *not* public API. Please let us know by
// opening a GitHub issue if your build environment requires some way to enable
// these cfgs other than by executing our build script.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rustc-cfg=if_docsrs_then_no_serde_core");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let patch_version = env::var("CARGO_PKG_VERSION_PATCH").unwrap();
    let module = PRIVATE.replace("$$", &patch_version);
    fs::write(out_dir.join("private.rs"), module).unwrap();

    let minor = match rustc_minor_version() {
        Some(minor) => minor,
        None => return,
    };

    if minor >= 77 {
        println!("cargo:rustc-check-cfg=cfg(feature, values(\"result\"))");
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

    // Current minimum supported version of serde_derive crate is Rust 1.61.
    if minor < 61 {
        println!("cargo:rustc-cfg=no_serde_derive");
    }

    // Support for the `#[diagnostic]` tool attribute namespace
    // https://blog.rust-lang.org/2024/05/02/Rust-1.78.0.html#diagnostic-attributes
    if minor < 78 {
        println!("cargo:rustc-cfg=no_diagnostic_namespace");
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
