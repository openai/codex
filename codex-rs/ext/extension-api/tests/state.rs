use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_extension_api::ExtensionData;
use pretty_assertions::assert_eq;

#[test]
fn typed_values_can_be_inserted_replaced_and_removed() {
    let data = ExtensionData::new("thread-1");

    assert_eq!(data.insert(/*value*/ 41_u64), None);
    assert_eq!(data.insert("alpha".to_string()), None);
    assert_eq!(data.get::<u64>().as_deref(), Some(&41));
    assert_eq!(
        data.get::<String>().map(|value| value.as_str().to_string()),
        Some("alpha".to_string())
    );

    assert_eq!(data.insert(/*value*/ 42_u64).as_deref(), Some(&41));
    assert_eq!(data.get::<u64>().as_deref(), Some(&42));
    assert_eq!(
        data.remove::<String>()
            .map(|value| value.as_str().to_string()),
        Some("alpha".to_string())
    );
    assert_eq!(data.get::<String>(), None);
    assert_eq!(data.get::<u64>().as_deref(), Some(&42));
}

#[test]
fn get_or_init_initializes_once_and_returns_shared_value() {
    #[derive(Debug, PartialEq, Eq)]
    struct SharedValue(usize);

    let data = Arc::new(ExtensionData::new("session"));
    let callers_started = Arc::new(AtomicUsize::new(0));
    let initialization_count = Arc::new(AtomicUsize::new(0));

    let handles: [_; 8] = std::array::from_fn(|_| {
        let data = Arc::clone(&data);
        let callers_started = Arc::clone(&callers_started);
        let initialization_count = Arc::clone(&initialization_count);
        std::thread::spawn(move || {
            callers_started.fetch_add(1, Ordering::SeqCst);
            data.get_or_init(|| {
                initialization_count.fetch_add(1, Ordering::SeqCst);
                while callers_started.load(Ordering::SeqCst) < 8 {
                    std::thread::yield_now();
                }
                SharedValue(7)
            })
        })
    });
    let values = handles
        .into_iter()
        .map(|handle| handle.join().expect("initializer thread should succeed"))
        .collect::<Vec<_>>();

    assert_eq!(initialization_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        values.iter().map(Arc::as_ref).collect::<Vec<_>>(),
        vec![&SharedValue(7); 8]
    );
    assert!(
        values
            .iter()
            .skip(1)
            .all(|value| Arc::ptr_eq(&values[0], value))
    );
}

#[test]
fn stores_are_isolated_and_preserve_level_id() {
    let session_data = ExtensionData::new("session-1");
    let thread_data = ExtensionData::new("thread-1");

    session_data.insert(/*value*/ 17_u32);
    thread_data.insert("thread value".to_string());

    assert_eq!(session_data.level_id(), "session-1");
    assert_eq!(thread_data.level_id(), "thread-1");
    assert_eq!(session_data.get::<u32>().as_deref(), Some(&17));
    assert_eq!(session_data.get::<String>(), None);
    assert_eq!(thread_data.get::<u32>(), None);
    assert_eq!(
        thread_data
            .get::<String>()
            .map(|value| value.as_str().to_string()),
        Some("thread value".to_string())
    );
}

#[test]
fn store_remains_usable_after_panicking_initializer() {
    let data = ExtensionData::new("turn-1");

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        data.get_or_init::<u64>(|| panic!("initializer failed"));
    }));

    assert!(result.is_err());
    assert_eq!(*data.get_or_init(|| 99_u64), 99);
}
