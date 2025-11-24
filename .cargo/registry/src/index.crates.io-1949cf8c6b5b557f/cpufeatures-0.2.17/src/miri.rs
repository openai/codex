//! Minimal miri support.
//!
//! Miri is an interpreter, and though it tries to emulate the target CPU
//! it does not support any target features.

#[macro_export]
#[doc(hidden)]
macro_rules! __unless_target_features {
    ($($tf:tt),+ => $body:expr ) => {
        false
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __detect_target_features {
    ($($tf:tt),+) => {
        false
    };
}
