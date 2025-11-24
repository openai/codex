#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(unix)]
#![cfg(not(miri))] // No `sigaction` on Miri.

use tokio::signal::unix::{signal, SignalKind};

#[cfg_attr(target_os = "wasi", ignore = "Wasi does not support panic recovery")]
#[test]
#[should_panic]
fn no_runtime_panics_creating_signals() {
    let _ = signal(SignalKind::hangup());
}
