#![allow(dead_code)]

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use hyper::rt::Sleep;

#[derive(Clone)]
pub(crate) struct Timer(Arc<dyn hyper::rt::Timer + Send + Sync>);

// =====impl Timer=====
impl Timer {
    pub(crate) fn new<T>(inner: T) -> Self
    where
        T: hyper::rt::Timer + Send + Sync + 'static,
    {
        Self(Arc::new(inner))
    }
}

impl fmt::Debug for Timer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Timer").finish()
    }
}

impl hyper::rt::Timer for Timer {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>> {
        self.0.sleep(duration)
    }

    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>> {
        self.0.sleep_until(deadline)
    }
}
