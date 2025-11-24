use std::panic;
use std::sync::{Arc, Mutex};

pub fn test_panic<Func: FnOnce() + panic::UnwindSafe>(func: Func) -> Option<String> {
    static PANIC_MUTEX: Mutex<()> = Mutex::new(());

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
                    .unwrap()
                    .clone_from(&Some(panic_location.file().to_string()));
            }));
        }

        let result = panic::catch_unwind(func);
        // Return to the previously set panic hook (maybe default) so that we get nice error
        // messages in the tests.
        panic::set_hook(prev_hook);

        if result.is_err() {
            panic_file.lock().unwrap().clone()
        } else {
            None
        }
    }
}
