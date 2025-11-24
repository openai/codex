#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;

#[cfg(not(unix))]
mod any;
#[cfg(not(unix))]
pub use any::*;
