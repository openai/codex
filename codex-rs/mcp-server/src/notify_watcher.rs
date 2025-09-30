#![allow(dead_code)]
//! Notification watcher bridge (feature-gated).

#[cfg(feature = "scheduler")]
pub struct NotificationWatcher;

#[cfg(feature = "scheduler")]
impl NotificationWatcher {
    pub fn new() -> Self { Self }
    pub fn spawn(self) {}
}

#[cfg(not(feature = "scheduler"))]
pub struct NotificationWatcher;

#[cfg(not(feature = "scheduler"))]
impl NotificationWatcher {
    pub fn new() -> Self { Self }
    pub fn spawn(self) {}
}

