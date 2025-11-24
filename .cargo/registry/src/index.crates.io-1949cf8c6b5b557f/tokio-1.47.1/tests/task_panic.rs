#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))]
#![cfg(panic = "unwind")]

use futures::future;
use std::error::Error;
use tokio::runtime::Builder;
use tokio::task::{self, block_in_place};

mod support {
    pub mod panic;
}
use support::panic::test_panic;

#[test]
fn block_in_place_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            block_in_place(|| {});
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn local_set_spawn_local_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let _local = task::LocalSet::new();

        task::spawn_local(async {});
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn local_set_block_on_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        let local = task::LocalSet::new();

        rt.block_on(async {
            local.block_on(&rt, future::pending::<()>());
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn spawn_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        tokio::spawn(future::pending::<()>());
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn local_key_sync_scope_panic_caller() -> Result<(), Box<dyn Error>> {
    tokio::task_local! {
        static NUMBER: u32;
    }

    let panic_location_file = test_panic(|| {
        NUMBER.sync_scope(1, || {
            NUMBER.with(|_| {
                NUMBER.sync_scope(1, || {});
            });
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn local_key_with_panic_caller() -> Result<(), Box<dyn Error>> {
    tokio::task_local! {
        static NUMBER: u32;
    }

    let panic_location_file = test_panic(|| {
        NUMBER.with(|_| {});
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn local_key_get_panic_caller() -> Result<(), Box<dyn Error>> {
    tokio::task_local! {
        static NUMBER: u32;
    }

    let panic_location_file = test_panic(|| {
        NUMBER.get();
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}
