use std::process::Command;
use std::str;

fn main() {
    // Temporary check to see if the rustc version >= 1.81 in which case the
    // `Error` trait is always available. This is temporary because in the
    // future the MSRV of this crate will be beyond 1.81 in which case this
    // build script can be deleted.
    let minor = rustc_minor_version().unwrap_or(0);
    if minor >= 81 {
        println!("cargo:rustc-cfg=core_error");
    }
    if minor >= 80 {
        println!("cargo:rustc-check-cfg=cfg(core_error)");
    }
}

fn rustc_minor_version() -> Option<u32> {
    let rustc = std::env::var("RUSTC").unwrap();
    let output = Command::new(rustc).arg("--version").output().ok()?;
    let version = str::from_utf8(&output.stdout).ok()?;
    let mut pieces = version.split('.');
    if pieces.next() != Some("rustc 1") {
        return None;
    }
    pieces.next()?.parse().ok()
}
