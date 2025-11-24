//! The `bitcast` and `bitflags_bits` macros.

#![allow(unused_macros)]

// Ensure that the source and destination types are both primitive integer
// types and the same size, and then bitcast.
macro_rules! bitcast {
    ($x:expr) => {{
        if false {
            // Ensure the source and destinations are primitive integer types.
            let _ = !$x;
            let _ = $x as u8;
            0
        } else if false {
            // Ensure that the source and destinations are the same size.
            // SAFETY: This code is under an `if false`.
            #[allow(
                unsafe_code,
                unused_unsafe,
                clippy::useless_transmute,
                clippy::missing_transmute_annotations
            )]
            unsafe {
                ::core::mem::transmute($x)
            }
        } else {
            // Do the conversion.
            $x as _
        }
    }};
}

/// Return a [`bitcast`] of the value of `$x.bits()`, where `$x` is a
/// `bitflags` type.
macro_rules! bitflags_bits {
    ($x:expr) => {{
        bitcast!($x.bits())
    }};
}
