//! Random-related operations.

#[cfg(linux_kernel)]
mod getrandom;

#[cfg(linux_kernel)]
pub use getrandom::{getrandom, GetRandomFlags};
