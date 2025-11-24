#![warn(rust_2018_idioms)]
#![cfg(all(feature = "time", not(target_os = "wasi")))] // Wasi does not support panic recovery
#![cfg(panic = "unwind")]

use parking_lot::{const_mutex, Mutex};
use std::error::Error;
use std::panic;
use std::sync::Arc;
use tokio::time::Duration;
use tokio_stream::{self as stream, StreamExt};

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
fn stream_chunks_timeout_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let iter = vec![1, 2, 3].into_iter();
        let stream0 = stream::iter(iter);

        let _chunk_stream = stream0.chunks_timeout(0, Duration::from_secs(2));
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}
