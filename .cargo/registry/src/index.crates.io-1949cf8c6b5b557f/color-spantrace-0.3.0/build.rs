use std::env;
use std::ffi::OsString;
use std::process::Command;
use std::str;

fn main() {
    let version = match rustc_version_info() {
        Some(version) => version,
        None => return,
    };
    version.toolchain.set_feature();
}

#[derive(PartialEq)]
enum Toolchain {
    Stable,
    Beta,
    Nightly,
}

impl Toolchain {
    fn set_feature(self) {
        println!("cargo:rustc-check-cfg=cfg(nightly)");
        println!("cargo:rustc-check-cfg=cfg(beta)");
        println!("cargo:rustc-check-cfg=cfg(stable)");
        match self {
            Toolchain::Nightly => println!("cargo:rustc-cfg=nightly"),
            Toolchain::Beta => println!("cargo:rustc-cfg=beta"),
            Toolchain::Stable => println!("cargo:rustc-cfg=stable"),
        }
    }
}

struct VersionInfo {
    toolchain: Toolchain,
}

fn rustc_version_info() -> Option<VersionInfo> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let output = Command::new(rustc).arg("--version").output().ok()?;
    let version = str::from_utf8(&output.stdout).ok()?;
    let mut pieces = version.split(['.', ' ', '-']);
    if pieces.next() != Some("rustc") {
        return None;
    }
    let _major: u32 = pieces.next()?.parse().ok()?;
    let _minor: u32 = pieces.next()?.parse().ok()?;
    let _patch: u32 = pieces.next()?.parse().ok()?;
    let toolchain = match pieces.next() {
        Some("beta") => Toolchain::Beta,
        Some("nightly") => Toolchain::Nightly,
        _ => Toolchain::Stable,
    };
    let version = VersionInfo { toolchain };
    Some(version)
}
