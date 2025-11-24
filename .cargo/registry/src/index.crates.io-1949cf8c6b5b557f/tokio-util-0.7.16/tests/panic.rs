#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support panic recovery
#![cfg(panic = "unwind")]

use parking_lot::{const_mutex, Mutex};
use std::error::Error;
use std::panic;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::channel;
use tokio::time::{Duration, Instant};
use tokio_test::task;
use tokio_util::io::SyncIoBridge;
use tokio_util::sync::PollSender;
use tokio_util::task::LocalPoolHandle;
use tokio_util::time::DelayQueue;

// Taken from tokio-util::time::wheel, if that changes then
const MAX_DURATION_MS: u64 = (1 << (36)) - 1;

fn test_panic<Func: FnOnce() + panic::UnwindSafe>(func: Func) -> Option<String> {
    static PANIC_MUTEX: Mutex<()> = const_mutex(());

    {
        let _guard = PANIC_MUTEX.lock();
        let panic_file: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let prev_hook = panic::take_hook();
        {
            let panic_file = panic_file.clone();
            panic::set_hook(Box::new(move |panic_info| {
                let panic_location = panic_info.location().unwrap();
                panic_file
                    .lock()
                    .clone_from(&Some(panic_location.file().to_string()));
            }));
        }

        let result = panic::catch_unwind(func);
        // Return to the previously set panic hook (maybe default) so that we get nice error
        // messages in the tests.
        panic::set_hook(prev_hook);

        if result.is_err() {
            panic_file.lock().clone()
        } else {
            None
        }
    }
}

#[test]
fn sync_bridge_new_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let _ = SyncIoBridge::new(tokio::io::empty());
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn poll_sender_send_item_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let (send, _) = channel::<u32>(3);
        let mut send = PollSender::new(send);

        let _ = send.send_item(42);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn local_pool_handle_new_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let _ = LocalPoolHandle::new(0);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn local_pool_handle_spawn_pinned_by_idx_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = basic();

        rt.block_on(async {
            let handle = LocalPoolHandle::new(2);
            handle.spawn_pinned_by_idx(|| async { "test" }, 3);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}
#[test]
fn delay_queue_insert_at_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = basic();
        rt.block_on(async {
            let mut queue = task::spawn(DelayQueue::with_capacity(3));

            //let st = std::time::Instant::from(SystemTime::UNIX_EPOCH);
            let _k = queue.insert_at(
                "1",
                Instant::now() + Duration::from_millis(MAX_DURATION_MS + 1),
            );
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn delay_queue_insert_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = basic();
        rt.block_on(async {
            let mut queue = task::spawn(DelayQueue::with_capacity(3));

            let _k = queue.insert("1", Duration::from_millis(MAX_DURATION_MS + 1));
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn delay_queue_remove_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = basic();
        rt.block_on(async {
            let mut queue = task::spawn(DelayQueue::with_capacity(3));

            let key = queue.insert_at("1", Instant::now());
            queue.remove(&key);
            queue.remove(&key);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn delay_queue_reset_at_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = basic();
        rt.block_on(async {
            let mut queue = task::spawn(DelayQueue::with_capacity(3));

            let key = queue.insert_at("1", Instant::now());
            queue.reset_at(
                &key,
                Instant::now() + Duration::from_millis(MAX_DURATION_MS + 1),
            );
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn delay_queue_reset_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = basic();
        rt.block_on(async {
            let mut queue = task::spawn(DelayQueue::with_capacity(3));

            let key = queue.insert_at("1", Instant::now());
            queue.reset(&key, Duration::from_millis(MAX_DURATION_MS + 1));
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn delay_queue_reserve_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = basic();
        rt.block_on(async {
            let mut queue = task::spawn(DelayQueue::<u32>::with_capacity(3));

            queue.reserve((1 << 30) as usize);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

fn basic() -> Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
