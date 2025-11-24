#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(unix)]
#![cfg(panic = "unwind")]
#![cfg(not(miri))] // No `sigaction` on Miri.

use std::error::Error;
use tokio::runtime::Builder;
use tokio::signal::unix::{signal, SignalKind};

mod support {
    pub mod panic;
}
use support::panic::test_panic;

#[test]
fn signal_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = Builder::new_current_thread().build().unwrap();

        rt.block_on(async {
            let kind = SignalKind::from_raw(-1);
            let _ = signal(kind);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}
