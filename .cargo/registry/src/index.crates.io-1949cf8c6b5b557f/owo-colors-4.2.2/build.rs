use std::{env, process::Command, str};

fn main() {
    // Once the MSRV is 1.74, we can replace this with a lint in Cargo.toml instead.
    println!("cargo:rustc-check-cfg=cfg(doc_cfg)");
    println!("cargo:rustc-check-cfg=cfg(const_mut_refs)");

    // Rust 1.83+ support &mut in const fn.
    if let Some(compiler) = rustc_version() {
        if compiler.is_newer_than_minor(83) {
            println!("cargo:rustc-cfg=const_mut_refs");
        }
    }
}

struct Compiler {
    minor: u32,
    channel: ReleaseChannel,
}

impl Compiler {
    fn is_newer_than_minor(&self, minor: u32) -> bool {
        match self.channel {
            ReleaseChannel::Stable | ReleaseChannel::Beta => self.minor >= minor,
            ReleaseChannel::Nightly => {
                // Early nightly versions might not have stabilized the feature
                // yet.
                self.minor > minor
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReleaseChannel {
    Stable,
    Beta,
    Nightly,
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
    let channel = if version.contains("nightly") {
        ReleaseChannel::Nightly
    } else if version.contains("beta") {
        ReleaseChannel::Beta
    } else {
        ReleaseChannel::Stable
    };
    Some(Compiler { minor, channel })
}
