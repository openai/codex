//! Error types

use std::{error, fmt};

/// The timeout elapsed.
#[derive(Debug, Default)]
pub struct Elapsed(pub(super) ());

impl Elapsed {
    /// Construct a new elapsed error
    pub const fn new() -> Self {
        Elapsed(())
    }
}

impl fmt::Display for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("request timed out")
    }
}

impl error::Error for Elapsed {}
