#[cfg(feature = "client")]
mod real;

#[cfg(not(feature = "client"))]
pub(crate) mod noop;

#[cfg(feature = "client")]
pub use self::real::*;

#[cfg(not(feature = "client"))]
pub use self::noop::*;
