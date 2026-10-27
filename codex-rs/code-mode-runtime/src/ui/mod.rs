//! Lightweight UI isolates share one bounded executor in the plugin process.
//! No ordinary CodeMode tools or globals.
//! Script failures discard their isolate without affecting its peers.

mod engine;
mod failures;
mod transport;
mod watchdog;
mod workers;

use std::io;

pub fn run() -> io::Result<()> {
    crate::initialize_v8(crate::V8JitMode::Disabled).map_err(io::Error::other)?;
    unsafe extern "C" {
        fn v8__V8__IsSandboxEnabled() -> bool;
    }
    // SAFETY: linked V8 exposes this query and has already been initialized.
    if !unsafe { v8__V8__IsSandboxEnabled() } {
        return Err(io::Error::other("sandbox-enabled V8 required"));
    }
    workers::run()
}
