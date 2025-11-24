//! Error types for the [`tower::balance`] middleware.
//!
//! [`tower::balance`]: crate::balance

use std::fmt;

/// The balancer's endpoint discovery stream failed.
#[derive(Debug)]
pub struct Discover(pub(crate) crate::BoxError);

impl fmt::Display for Discover {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "load balancer discovery error: {}", self.0)
    }
}

impl std::error::Error for Discover {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.0)
    }
}
