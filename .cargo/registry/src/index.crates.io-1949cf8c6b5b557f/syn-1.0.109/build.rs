use std::env;
use std::process::Command;
use std::str;

// The rustc-cfg strings below are *not* public API. Please let us know by
// opening a GitHub issue if your build environment requires some way to enable
// these cfgs other than by executing our build script.
fn main() {
    let compiler = match rustc_version() {
        Some(compiler) => compiler,
        None => return,
    };

    if compiler.minor < 36 {
        println!("cargo:rustc-cfg=syn_omit_await_from_token_macro");
    }

    if compiler.minor < 39 {
        println!("cargo:rustc-cfg=syn_no_const_vec_new");
    }

    if compiler.minor < 40 {
        println!("cargo:rustc-cfg=syn_no_non_exhaustive");
    }

    if compiler.minor < 56 {
        println!("cargo:rustc-cfg=syn_no_negative_literal_parse");
    }

    if !compiler.nightly {
        println!("cargo:rustc-cfg=syn_disable_nightly_tests");
    }
}

struct Compiler {
    minor: u32,
    nightly: bool,
}

fn rustc_version() -> Option<Compiler> {
    let rustc = env::var_os("RUSTC")?;
    let output = Command::new(rustc).arg("--version").output().ok()?;
    let version = str::from_utf8(&output.stdout).ok()?;
    let mut pieces = version.split('.');
    if pieces.next() != Some("rustc 1") {
        return None;
    }
    let minor = pieces.next()?.parse().ok()?;
    let nightly = version.contains("nightly") || version.ends_with("-dev");
    Some(Compiler { minor, nightly })
}
