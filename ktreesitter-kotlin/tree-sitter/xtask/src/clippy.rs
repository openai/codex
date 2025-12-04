use std::process::Command;

use anyhow::Result;

use crate::{bail_on_err, Clippy};

pub fn run(args: &Clippy) -> Result<()> {
    let mut clippy_command = Command::new("cargo");
    clippy_command.arg("clippy");

    if let Some(package) = args.package.as_ref() {
        clippy_command.args(["--package", package]);
    } else {
        clippy_command.arg("--workspace");
    }

    clippy_command
        .arg("--release")
        .arg("--all-targets")
        .arg("--all-features")
        .arg("--")
        .arg("-D")
        .arg("warnings");

    if args.fix {
        clippy_command.arg("--fix");
    }

    bail_on_err(
        &clippy_command.spawn()?.wait_with_output()?,
        "Clippy failed",
    )
}
