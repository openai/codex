// Tests that depend on a count of the number of times their filter is evaluated
// can't exist in the same file with other tests that add subscribers to the
// registry. The registry was changed so that each time a new dispatcher is
// added all filters are re-evaluated. The tests being run only in separate
// threads with shared global state lets them interfere with each other

#[cfg(not(feature = "std"))]
extern crate std;

use tracing::{span, Level};
use tracing_mock::*;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn filter_caching_is_lexically_scoped() {
    pub fn my_great_function() -> bool {
        span!(Level::TRACE, "emily").in_scope(|| true)
    }

    pub fn my_other_function() -> bool {
        span!(Level::TRACE, "frank").in_scope(|| true)
    }

    let count = Arc::new(AtomicUsize::new(0));
    let count2 = count.clone();

    let subscriber = subscriber::mock()
        .with_filter(move |meta| match meta.name() {
            "emily" | "frank" => {
                count2.fetch_add(1, Ordering::Relaxed);
                true
            }
            _ => false,
        })
        .run();

    // Since this test is in its own file anyway, we can do this. Thus, this
    // test will work even with no-std.
    tracing::subscriber::set_global_default(subscriber).unwrap();

    // Call the function once. The filter should be re-evaluated.
    assert!(my_great_function());
    assert_eq!(count.load(Ordering::Relaxed), 1);

    // Call the function again. The cached result should be used.
    assert!(my_great_function());
    assert_eq!(count.load(Ordering::Relaxed), 1);

    assert!(my_other_function());
    assert_eq!(count.load(Ordering::Relaxed), 2);

    assert!(my_great_function());
    assert_eq!(count.load(Ordering::Relaxed), 2);

    assert!(my_other_function());
    assert_eq!(count.load(Ordering::Relaxed), 2);

    assert!(my_great_function());
    assert_eq!(count.load(Ordering::Relaxed), 2);
}
