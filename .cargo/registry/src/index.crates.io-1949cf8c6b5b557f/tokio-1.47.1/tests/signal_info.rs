#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "illumos"
))]
#![cfg(not(miri))] // No `sigaction` on Miri

mod support {
    pub mod signal;
}
use support::signal::send_signal;

use tokio::signal;
use tokio::signal::unix::SignalKind;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn siginfo() {
    let mut sig = signal::unix::signal(SignalKind::info()).expect("installed signal handler");

    tokio::spawn(async {
        send_signal(libc::SIGINFO);
    });

    // Add a timeout to ensure the test doesn't hang.
    timeout(Duration::from_secs(5), sig.recv())
        .await
        .expect("received SIGINFO signal in time")
        .expect("received SIGINFO signal");
}
