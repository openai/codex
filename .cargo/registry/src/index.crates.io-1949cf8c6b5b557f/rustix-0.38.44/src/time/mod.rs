//! Time-related operations.

mod clock;
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
mod timerfd;

// TODO: Convert WASI'S clock APIs to use handles rather than ambient clock
// identifiers, update `wasi-libc`, and then add support in `rustix`.
pub use clock::*;
#[cfg(any(linux_kernel, target_os = "fuchsia"))]
pub use timerfd::*;
