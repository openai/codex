// The rustc-cfg listed below are considered public API, but it is *unstable*
// and outside of the normal semver guarantees:
//
// - `crossbeam_no_atomic`
//      Assume the target does *not* support any atomic operations.
//      This is usually detected automatically by the build script, but you may
//      need to enable it manually when building for custom targets or using
//      non-cargo build systems that don't run the build script.
//
// With the exceptions mentioned above, the rustc-cfg emitted by the build
// script are *not* public API.

#![warn(rust_2018_idioms)]

use std::env;

include!("no_atomic.rs");
include!("build-common.rs");

fn main() {
    println!("cargo:rerun-if-changed=no_atomic.rs");
    println!("cargo:rustc-check-cfg=cfg(crossbeam_no_atomic,crossbeam_sanitize_thread)");

    let target = match env::var("TARGET") {
        Ok(target) => convert_custom_linux_target(target),
        Err(e) => {
            println!(
                "cargo:warning={}: unable to get TARGET environment variable: {}",
                env!("CARGO_PKG_NAME"),
                e
            );
            return;
        }
    };

    // Note that this is `no_`*, not `has_*`. This allows treating as the latest
    // stable rustc is used when the build script doesn't run. This is useful
    // for non-cargo build systems that don't run the build script.
    if NO_ATOMIC.contains(&&*target) {
        println!("cargo:rustc-cfg=crossbeam_no_atomic");
    }

    // `cfg(sanitize = "..")` is not stabilized.
    let sanitize = env::var("CARGO_CFG_SANITIZE").unwrap_or_default();
    if sanitize.contains("thread") {
        println!("cargo:rustc-cfg=crossbeam_sanitize_thread");
    }
}
