use std::{error, fmt};

/// Error returned if the inner [`Service`] has not been set.
///
/// [`Service`]: crate::Service
#[derive(Debug)]
pub struct None(());

impl None {
    pub(crate) fn new() -> None {
        None(())
    }
}

impl fmt::Display for None {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "None")
    }
}

impl error::Error for None {}
