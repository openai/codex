use std::env;
use std::process::Command;
use std::str;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let minor = match rustc_minor_version() {
        Some(minor) => minor,
        None => return,
    };

    if minor >= 80 {
        println!("cargo:rustc-check-cfg=cfg(no_intrinsic_type_name)");
        println!("cargo:rustc-check-cfg=cfg(no_phantom_pinned)");
    }

    if minor < 33 {
        println!("cargo:rustc-cfg=no_phantom_pinned");
    }

    if minor < 38 {
        println!("cargo:rustc-cfg=no_intrinsic_type_name");
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
