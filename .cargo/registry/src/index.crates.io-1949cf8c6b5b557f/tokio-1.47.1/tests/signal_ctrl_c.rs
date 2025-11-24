#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(unix)]
#![cfg(not(miri))] // No `sigaction` on Miri

mod support {
    pub mod signal;
}
use support::signal::send_signal;

use tokio::signal;
use tokio_test::assert_ok;

#[tokio::test]
async fn ctrl_c() {
    let ctrl_c = signal::ctrl_c();

    tokio::spawn(async {
        send_signal(libc::SIGINT);
    });

    assert_ok!(ctrl_c.await);
}
