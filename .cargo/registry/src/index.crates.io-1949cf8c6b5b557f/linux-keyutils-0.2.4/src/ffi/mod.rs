#[allow(dead_code)]
mod functions;
mod types;

macro_rules! keyctl {
    ( $op:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr ) => {
        $crate::ffi::keyctl_impl($op, $a2, $a3, $a4, $a5)
    };
    ( $op:expr, $a2:expr, $a3:expr, $a4:expr) => {
        $crate::ffi::keyctl_impl($op, $a2, $a3, $a4, 0)
    };
    ( $op:expr, $a2:expr, $a3:expr ) => {
        $crate::ffi::keyctl_impl($op, $a2, $a3, 0, 0)
    };
    ( $op:expr, $a2:expr ) => {
        $crate::ffi::keyctl_impl($op, $a2, 0, 0, 0)
    };
}

#[allow(unused_imports)]
pub use types::*;

#[allow(unused_imports)]
pub(crate) use functions::{add_key, keyctl_impl};

// Export the macro for use
pub(crate) use keyctl;
