//! Conversion between units of time.

use self::sealed::{DefaultOutput, MultipleOf};

mod sealed {
    /// A trait for defining the ratio of two units of time.
    ///
    /// This trait is used to implement the `per` method on the various structs.
    #[diagnostic::on_unimplemented(message = "`{Self}` is not an integer multiple of `{T}`")]
    pub trait MultipleOf<T, Output> {
        /// The number of one unit of time in the other.
        const VALUE: Output;
    }

    pub trait DefaultOutput<T> {
        type Output;
    }
}

/// Given the list of types, stringify them as a list.
macro_rules! stringify_outputs {
    (@inner $first:ty) => {
        concat!("or `", stringify!($first), "`")
    };
    (@inner $first:ty, $($t:ty),+) => {
        concat!(stringify_outputs!($first), ", ", stringify_outputs!(@inner $($t),+))
    };
    ($first:ty) => {
        concat!("`", stringify!($first), "`")
    };
    ($($t:ty),+) => {
        stringify_outputs!(@inner $($t),+)
    };
}

// Split this out to a separate function to permit naming `T` while also using `impl Trait` as a
// parameter in the public API.`
const fn multiple_of_value<T, U, Output>(_: T) -> Output
where
    T: MultipleOf<U, Output> + Copy,
{
    T::VALUE
}

/// Declare and implement `Per` for all relevant types. Identity implementations are automatic.
macro_rules! impl_per {
    ($($t:ident ($str:literal) per {$(
        $larger:ident : [$default_output:ty]

        $($int_output:ty)|+ = $int_value:expr;
        $($float_output:ty)|+ = $float_value:expr;
    )+})*) => {$(
        #[doc = concat!("A unit of time representing exactly one ", $str, ".")]
        #[derive(Debug, Clone, Copy)]
        pub struct $t;

        impl $t {
            #[doc = concat!("Obtain the number of times `", stringify!($t), "` can fit into `T`.")]
            #[doc = concat!("If `T` is smaller than `", stringify!($t), "`, the code will fail to")]
            /// compile. The return type is the smallest unsigned integer type that can represent
            /// the value.
            ///
            /// Valid calls:
            ///
            $(#[doc = concat!(
                "  - `", stringify!($t), "::per(", stringify!($larger), ")` (returns `",
                stringify!($default_output), "`)"
            )])+
            #[inline]
            pub const fn per<T>(_larger: T) -> <T as DefaultOutput<Self>>::Output
            where
                T: MultipleOf<Self, T::Output> + DefaultOutput<Self> + Copy,
            {
                T::VALUE
            }

            #[doc = concat!("Obtain the number of times `", stringify!($t), "` can fit into `T`.")]
            #[doc = concat!("If `T` is smaller than `", stringify!($t), "`, the code will fail to")]
            /// compile. The return type is any primitive numeric type that can represent the value.
            ///
            /// Valid calls:
            ///
            $(#[doc = concat!(
                "  - `", stringify!($t), "::per(", stringify!($larger), ")` (returns ",
                stringify_outputs!($($int_output),+ , $($float_output),+), ")"
            )])+
            #[inline]
            pub const fn per_t<Output>(larger: impl MultipleOf<Self, Output> + Copy) -> Output {
                multiple_of_value(larger)
            }
        }

        $(
            $(impl MultipleOf<$t, $int_output> for $larger {
                const VALUE: $int_output = $int_value;
            })+

            $(impl MultipleOf<$t, $float_output> for $larger {
                const VALUE: $float_output = $float_value;
            })+

            impl DefaultOutput<$t> for $larger {
                type Output = $default_output;
            }
        )+
    )*};
}

impl_per! {
    Nanosecond ("nanosecond") per {
        Nanosecond: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 1; f32|f64 = 1.;
        Microsecond: [u16] u16|u32|u64|u128|usize|i16|i32|i64|i128|isize = 1_000; f32|f64 = 1_000.;
        Millisecond: [u32] u32|u64|u128|usize|i32|i64|i128|isize = 1_000_000; f32|f64 = 1_000_000.;
        Second:
            [u32] u32|u64|u128|usize|i32|i64|i128|isize = 1_000_000_000; f32|f64 = 1_000_000_000.;
        Minute: [u64] u64|u128|i64|i128 = 60_000_000_000; f32|f64 = 60_000_000_000.;
        Hour: [u64] u64|u128|i64|i128 = 3_600_000_000_000; f32|f64 = 3_600_000_000_000.;
        Day: [u64] u64|u128|i64|i128 = 86_400_000_000_000; f32|f64 = 86_400_000_000_000.;
        Week: [u64] u64|u128|i64|i128 = 604_800_000_000_000; f32|f64 = 604_800_000_000_000.;
    }
    Microsecond ("microsecond") per {
        Microsecond: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 1; f32|f64 = 1.;
        Millisecond: [u16] u16|u32|u64|u128|usize|i16|i32|i64|i128|isize = 1_000; f32|f64 = 1_000.;
        Second: [u32] u32|u64|u128|usize|i32|i64|i128|isize = 1_000_000; f32|f64 = 1_000_000.;
        Minute: [u32] u32|u64|u128|usize|i32|i64|i128|isize = 60_000_000; f32|f64 = 60_000_000.;
        Hour: [u32] u32|u64|u128|i64|i128 = 3_600_000_000; f32|f64 = 3_600_000_000.;
        Day: [u64] u64|u128|i64|i128 = 86_400_000_000; f32|f64 = 86_400_000_000.;
        Week: [u64] u64|u128|i64|i128 = 604_800_000_000; f32|f64 = 604_800_000_000.;
    }
    Millisecond ("millisecond") per {
        Millisecond: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 1; f32|f64 = 1.;
        Second: [u16] u16|u32|u64|u128|usize|i16|i32|i64|i128|isize = 1_000; f32|f64 = 1_000.;
        Minute: [u16] u16|u32|u64|u128|usize|i32|i64|i128|isize = 60_000; f32|f64 = 60_000.;
        Hour: [u32] u32|u64|u128|usize|i32|i64|i128|isize = 3_600_000; f32|f64 = 3_600_000.;
        Day: [u32] u32|u64|u128|usize|i32|i64|i128|isize = 86_400_000; f32|f64 = 86_400_000.;
        Week: [u32] u32|u64|u128|usize|i32|i64|i128|isize = 604_800_000; f32|f64 = 604_800_000.;
    }
    Second ("second") per {
        Second: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 1; f32|f64 = 1.;
        Minute: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 60; f32|f64 = 60.;
        Hour: [u16] u16|u32|u64|u128|usize|i16|i32|i64|i128|isize = 3_600; f32|f64 = 3_600.;
        Day: [u32] u32|u64|u128|usize|i32|i64|i128|isize = 86_400; f32|f64 = 86_400.;
        Week: [u32] u32|u64|u128|usize|i32|i64|i128|isize = 604_800; f32|f64 = 604_800.;
    }
    Minute ("minute") per {
        Minute: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 1; f32|f64 = 1.;
        Hour: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 60; f32|f64 = 60.;
        Day: [u16] u16|u32|u64|u128|usize|i16|i32|i64|i128|isize = 1_440; f32|f64 = 1_440.;
        Week: [u16] u16|u32|u64|u128|usize|i16|i32|i64|i128|isize = 10_080; f32|f64 = 10_080.;
    }
    Hour ("hour") per {
        Hour: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 1; f32|f64 = 1.;
        Day: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 24; f32|f64 = 24.;
        Week: [u8] u8|u16|u32|u64|u128|usize|i16|i32|i64|i128|isize = 168; f32|f64 = 168.;
    }
    Day ("day") per {
        Day: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 1; f32|f64 = 1.;
        Week: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 7; f32|f64 = 7.;
    }
    Week ("week") per {
        Week: [u8] u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize = 1; f32|f64 = 1.;
    }
}
