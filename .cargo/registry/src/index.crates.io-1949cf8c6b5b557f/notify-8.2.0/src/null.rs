//! Stub Watcher implementation

#![allow(unused_variables)]

use crate::Config;

use super::{RecursiveMode, Result, Watcher};
use std::path::Path;

/// Stub `Watcher` implementation
///
/// Events are never delivered from this watcher.
#[derive(Debug)]
pub struct NullWatcher;

impl Watcher for NullWatcher {
    fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        Ok(())
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        Ok(())
    }

    fn new<F: crate::EventHandler>(event_handler: F, config: Config) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(NullWatcher)
    }

    fn configure(&mut self, config: Config) -> Result<bool> {
        Ok(false)
    }

    fn kind() -> crate::WatcherKind {
        crate::WatcherKind::NullWatcher
    }
}
