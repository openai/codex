#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(unix)]
#![cfg(not(miri))] // No `sigaction` on Miri.

mod support {
    pub mod signal;
}
use support::signal::send_signal;

use tokio::signal::unix::{signal, SignalKind};

#[tokio::test]
async fn twice() {
    let kind = SignalKind::user_defined1();
    let mut sig = signal(kind).expect("failed to get signal");

    for _ in 0..2 {
        send_signal(libc::SIGUSR1);

        assert!(sig.recv().await.is_some());
    }
}
