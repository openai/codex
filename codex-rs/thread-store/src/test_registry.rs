use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;

use crate::ThreadStore;

static TEST_THREAD_STORES: OnceLock<Mutex<HashMap<String, Arc<dyn ThreadStore>>>> = OnceLock::new();

fn stores() -> &'static Mutex<HashMap<String, Arc<dyn ThreadStore>>> {
    // The registry is global process state used by integration tests while the
    // app-server runs worker tasks concurrently, so access needs synchronization
    // even though each individual test usually registers only one endpoint.
    TEST_THREAD_STORES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stores_guard() -> MutexGuard<'static, HashMap<String, Arc<dyn ThreadStore>>> {
    match stores().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Registers a test-only thread store for a synthetic endpoint.
///
/// This lets tests exercise config-driven non-local thread persistence without
/// requiring the real remote gRPC service.
pub fn register_test_thread_store(endpoint: impl Into<String>, store: Arc<dyn ThreadStore>) {
    stores_guard().insert(endpoint.into(), store);
}

/// Removes a previously registered test-only thread store.
pub fn remove_test_thread_store(endpoint: &str) -> Option<Arc<dyn ThreadStore>> {
    stores_guard().remove(endpoint)
}

/// Returns a registered test-only thread store for `endpoint`, if present.
pub fn test_thread_store(endpoint: &str) -> Option<Arc<dyn ThreadStore>> {
    stores_guard().get(endpoint).cloned()
}
