#![allow(clippy::module_name_repetitions)]

use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct Flag {
    atomic: Arc<AtomicBool>,
}

impl Flag {
    pub fn new() -> Self {
        Flag {
            atomic: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn get(&self) -> bool {
        self.atomic.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub struct DetectDrop {
    has_dropped: Flag,
}

impl DetectDrop {
    pub fn new(has_dropped: &Flag) -> Self {
        DetectDrop {
            has_dropped: Flag {
                atomic: Arc::clone(&has_dropped.atomic),
            },
        }
    }
}

impl StdError for DetectDrop {}

impl Display for DetectDrop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "oh no!")
    }
}

impl Drop for DetectDrop {
    fn drop(&mut self) {
        let already_dropped = self.has_dropped.atomic.swap(true, Ordering::Relaxed);
        assert!(!already_dropped);
    }
}
