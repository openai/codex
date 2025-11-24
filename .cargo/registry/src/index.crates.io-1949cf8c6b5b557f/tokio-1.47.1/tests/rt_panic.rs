#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(not(target_os = "wasi"))] // Wasi doesn't support panic recovery
#![cfg(panic = "unwind")]

use futures::future;
use std::error::Error;
use tokio::runtime::{Builder, Handle, Runtime};

mod support {
    pub mod panic;
}
use support::panic::test_panic;

#[test]
fn current_handle_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let _ = Handle::current();
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn into_panic_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(move || {
        let rt = current_thread();
        rt.block_on(async {
            let handle = tokio::spawn(future::pending::<()>());

            handle.abort();

            let err = handle.await.unwrap_err();
            assert!(!&err.is_panic());

            let _ = err.into_panic();
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn builder_worker_threads_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let _ = Builder::new_multi_thread().worker_threads(0).build();
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn builder_max_blocking_threads_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let _ = Builder::new_multi_thread().max_blocking_threads(0).build();
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn builder_global_queue_interval_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let _ = Builder::new_multi_thread().global_queue_interval(0).build();
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

fn current_thread() -> Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
