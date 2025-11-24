use crate::propagation::TextMapPropagator;
use crate::trace::noop::NoopTextMapPropagator;
use std::sync::{OnceLock, RwLock};

/// The current global `TextMapPropagator` propagator.
static GLOBAL_TEXT_MAP_PROPAGATOR: OnceLock<RwLock<Box<dyn TextMapPropagator + Send + Sync>>> =
    OnceLock::new();

/// The global default `TextMapPropagator` propagator.
static DEFAULT_TEXT_MAP_PROPAGATOR: OnceLock<NoopTextMapPropagator> = OnceLock::new();

/// Ensures the `GLOBAL_TEXT_MAP_PROPAGATOR` is initialized with a `NoopTextMapPropagator`.
#[inline]
fn global_text_map_propagator() -> &'static RwLock<Box<dyn TextMapPropagator + Send + Sync>> {
    GLOBAL_TEXT_MAP_PROPAGATOR.get_or_init(|| RwLock::new(Box::new(NoopTextMapPropagator::new())))
}

/// Ensures the `DEFAULT_TEXT_MAP_PROPAGATOR` is initialized.
#[inline]
fn default_text_map_propagator() -> &'static NoopTextMapPropagator {
    DEFAULT_TEXT_MAP_PROPAGATOR.get_or_init(NoopTextMapPropagator::new)
}

/// Sets the given [`TextMapPropagator`] propagator as the current global propagator.
pub fn set_text_map_propagator<P: TextMapPropagator + Send + Sync + 'static>(propagator: P) {
    let _lock = global_text_map_propagator()
        .write()
        .map(|mut global_propagator| *global_propagator = Box::new(propagator));
}

/// Executes a closure with a reference to the current global [`TextMapPropagator`] propagator.
pub fn get_text_map_propagator<T, F>(mut f: F) -> T
where
    F: FnMut(&dyn TextMapPropagator) -> T,
{
    global_text_map_propagator()
        .read()
        .map(|propagator| f(&**propagator))
        .unwrap_or_else(|_| {
            let default_propagator = default_text_map_propagator();
            f(default_propagator as &dyn TextMapPropagator)
        })
}
