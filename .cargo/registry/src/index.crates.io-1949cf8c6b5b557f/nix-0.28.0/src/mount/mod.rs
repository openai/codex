//! Mount file systems
#[cfg(linux_android)]
mod linux;

#[cfg(linux_android)]
pub use self::linux::*;

#[cfg(bsd)]
mod bsd;

#[cfg(bsd)]
pub use self::bsd::*;
