use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use rustc_version::{version, version_meta, Channel};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("constants.gen.rs");
    let mut f = File::create(dest_path)?;

    let target = env::var("TARGET")?;
    let mut target_bits = target.split('-');
    // `NoneError` does not implement `std::error::Error`
    // See https://github.com/rust-lang/rust/issues/46871
    let arch = target_bits.next().unwrap();
    target_bits.next();
    let platform = target_bits.next().unwrap();

    writeln!(
        f,
        "/// The rustc version that was used to compile this crate."
    )?;
    writeln!(
        f,
        "pub const RUSTC_VERSION: Option<&'static str> = {};",
        if let Ok(version) = version() {
            format!("Some(\"{version}\")")
        } else {
            "None".into()
        }
    )?;
    writeln!(
        f,
        "/// The rustc release channel that was used to compile this crate."
    )?;
    writeln!(
        f,
        "pub const RUSTC_CHANNEL: Option<&'static str> = {};",
        if let Ok(version_meta) = version_meta() {
            let chan = match version_meta.channel {
                Channel::Dev => "dev",
                Channel::Nightly => "nightly",
                Channel::Beta => "beta",
                Channel::Stable => "stable",
            };
            format!("Some(\"{chan}\")")
        } else {
            "None".into()
        }
    )?;

    writeln!(f, "/// The platform identifier.")?;
    writeln!(
        f,
        "#[allow(unused)] pub const PLATFORM: &str = \"{platform}\";"
    )?;
    writeln!(f, "/// The CPU architecture identifier.")?;
    writeln!(f, "pub const ARCH: &str = \"{arch}\";")?;

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    Ok(())
}
