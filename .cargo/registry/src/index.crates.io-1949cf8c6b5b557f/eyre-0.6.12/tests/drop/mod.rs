use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
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
        self.atomic.load(SeqCst)
    }
}

#[derive(Debug)]
pub struct DetectDrop {
    has_dropped: Flag,
    label: &'static str,
}

impl DetectDrop {
    pub fn new(label: &'static str, has_dropped: &Flag) -> Self {
        DetectDrop {
            label,
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
        eprintln!("Dropping {}", self.label);
        let already_dropped = self.has_dropped.atomic.swap(true, SeqCst);
        assert!(!already_dropped);
    }
}
