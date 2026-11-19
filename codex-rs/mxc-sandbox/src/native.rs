//! Run an already prepared MXC request with inherited stdio and job ownership.

use std::time::Instant;

use crate::windows::log_launch;
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
    let spawning = Instant::now();
    let mut child = v1::spawn_with_extended_options(request, options)?;
    let spawn_ms = spawning.elapsed().as_millis();
    log_launch(format_args!("phase=wait spawn_ms={spawn_ms}"));
    let waiting = Instant::now();
    let outcome = child.wait().context("waiting for the MXC command")?;
    let wait_ms = waiting.elapsed().as_millis();
    log_launch(format_args!(
        "phase=finished outcome={outcome:?} wait_ms={wait_ms}"
    ));
    match outcome {
        v1::WaitResult::Exited(code) => Ok(code),
        v1::WaitResult::TimedOut => anyhow::bail!("MXC command timed out"),
    }
}
