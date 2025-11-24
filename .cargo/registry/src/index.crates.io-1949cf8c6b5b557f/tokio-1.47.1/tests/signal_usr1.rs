#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(unix)]
#![cfg(not(miri))] // No `sigaction` in Miri.

mod support {
    pub mod signal;
}
use support::signal::send_signal;

use tokio::signal::unix::{signal, SignalKind};
use tokio_test::assert_ok;

#[tokio::test]
async fn signal_usr1() {
    let mut signal = assert_ok!(
        signal(SignalKind::user_defined1()),
        "failed to create signal"
    );

    send_signal(libc::SIGUSR1);

    signal.recv().await;
}
