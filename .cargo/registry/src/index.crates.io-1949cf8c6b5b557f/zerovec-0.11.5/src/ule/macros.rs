// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

/// Given `Self` (`$aligned`), `Self::ULE` (`$unaligned`), and a conversion function (`$single` or
/// `Self::from_aligned`), implement `from_array` for arrays of `$aligned` to `$unaligned`.
///
/// The `$default` argument is due to current compiler limitations.
/// Pass any (cheap to construct) value.
#[macro_export]
macro_rules! impl_ule_from_array {
    ($aligned:ty, $unaligned:ty, $default:expr, $single:path) => {
        #[doc = concat!("Convert an array of `", stringify!($aligned), "` to an array of `", stringify!($unaligned), "`.")]
        pub const fn from_array<const N: usize>(arr: [$aligned; N]) -> [Self; N] {
            let mut result = [$default; N];
            let mut i = 0;
            // Won't panic because i < N and arr has length N
            #[expect(clippy::indexing_slicing)]
            while i < N {
                result[i] = $single(arr[i]);
                i += 1;
            }
            result
        }
    };
    ($aligned:ty, $unaligned:ty, $default:expr) => {
        impl_ule_from_array!($aligned, $unaligned, $default, Self::from_aligned);
    };
}
