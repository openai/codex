//! Run an already prepared MXC request with inherited stdio and job ownership.

use anyhow::Context;
use anyhow::Result;
use mxc_sdk::v1;

// The ETW functions used by tracelogging are documented Advapi32 exports on
// both Windows toolchains; 1.2.3 leaves their import library to the application.
#[link(name = "advapi32")]
unsafe extern "system" {}

pub(super) fn launch(request: v1::ContainerRequest) -> Result<i32> {
    let mut options = v1::SpawnExtendedOptions::default();
    options.stdio = v1::SpawnStdio::Inherit;
    let mut child = v1::spawn_with_extended_options(request, options)?;
    match child.wait().context("waiting for the MXC command")? {
        v1::WaitResult::Exited(code) => Ok(code),
        v1::WaitResult::TimedOut => anyhow::bail!("MXC command timed out"),
    }
}
