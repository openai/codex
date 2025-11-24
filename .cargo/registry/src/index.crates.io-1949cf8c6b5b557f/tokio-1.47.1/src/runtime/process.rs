#![cfg_attr(not(feature = "rt"), allow(dead_code))]

//! Process driver.

use crate::process::unix::GlobalOrphanQueue;
use crate::runtime::driver;
use crate::runtime::signal::{Driver as SignalDriver, Handle as SignalHandle};

use std::time::Duration;

/// Responsible for cleaning up orphaned child processes on Unix platforms.
#[derive(Debug)]
pub(crate) struct Driver {
    park: SignalDriver,
    signal_handle: SignalHandle,
}

// ===== impl Driver =====

impl Driver {
    /// Creates a new signal `Driver` instance that delegates wakeups to `park`.
    pub(crate) fn new(park: SignalDriver) -> Self {
        let signal_handle = park.handle();

        Self {
            park,
            signal_handle,
        }
    }

    pub(crate) fn park(&mut self, handle: &driver::Handle) {
        self.park.park(handle);
        GlobalOrphanQueue::reap_orphans(&self.signal_handle);
    }

    pub(crate) fn park_timeout(&mut self, handle: &driver::Handle, duration: Duration) {
        self.park.park_timeout(handle, duration);
        GlobalOrphanQueue::reap_orphans(&self.signal_handle);
    }

    pub(crate) fn shutdown(&mut self, handle: &driver::Handle) {
        self.park.shutdown(handle);
    }
}
