use core::borrow::{Borrow, BorrowMut};
use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
use core::fmt::Debug;
use core::hash::Hash;

pub trait NumBytes:
    Debug
    + AsRef<[u8]>
    + AsMut<[u8]>
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + Hash
    + Borrow<[u8]>
    + BorrowMut<[u8]>
{
}

impl<T> NumBytes for T where
    T: Debug
        + AsRef<[u8]>
        + AsMut<[u8]>
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Borrow<[u8]>
        + BorrowMut<[u8]>
        + ?Sized
{
}

pub trait ToBytes {
    type Bytes: NumBytes;

    /// Return the memory representation of this number as a byte array in big-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::ToBytes;
    ///
    /// let bytes = ToBytes::to_be_bytes(&0x12345678u32);
    /// assert_eq!(bytes, [0x12, 0x34, 0x56, 0x78]);
    /// ```
    fn to_be_bytes(&self) -> Self::Bytes;

    /// Return the memory representation of this number as a byte array in little-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::ToBytes;
    ///
    /// let bytes = ToBytes::to_le_bytes(&0x12345678u32);
    /// assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);
    /// ```
    fn to_le_bytes(&self) -> Self::Bytes;

    /// Return the memory representation of this number as a byte array in native byte order.
    ///
    /// As the target platform's native endianness is used,
    /// portable code should use [`to_be_bytes`] or [`to_le_bytes`], as appropriate, instead.
    ///
    /// [`to_be_bytes`]: #method.to_be_bytes
    /// [`to_le_bytes`]: #method.to_le_bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::ToBytes;
    ///
    /// #[cfg(target_endian = "big")]
    /// let expected = [0x12, 0x34, 0x56, 0x78];
    ///
    /// #[cfg(target_endian = "little")]
    /// let expected = [0x78, 0x56, 0x34, 0x12];
    ///
    /// let bytes = ToBytes::to_ne_bytes(&0x12345678u32);
    /// assert_eq!(bytes, expected)
    /// ```
    fn to_ne_bytes(&self) -> Self::Bytes {
        #[cfg(target_endian = "big")]
        let bytes = self.to_be_bytes();
        #[cfg(target_endian = "little")]
        let bytes = self.to_le_bytes();
        bytes
    }
}

pub trait FromBytes: Sized {
    type Bytes: NumBytes + ?Sized;

    /// Create a number from its representation as a byte array in big endian.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::FromBytes;
    ///
    /// let value: u32 = FromBytes::from_be_bytes(&[0x12, 0x34, 0x56, 0x78]);
    /// assert_eq!(value, 0x12345678);
    /// ```
    fn from_be_bytes(bytes: &Self::Bytes) -> Self;

    /// Create a number from its representation as a byte array in little endian.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::FromBytes;
    ///
    /// let value: u32 = FromBytes::from_le_bytes(&[0x78, 0x56, 0x34, 0x12]);
    /// assert_eq!(value, 0x12345678);
    /// ```
    fn from_le_bytes(bytes: &Self::Bytes) -> Self;

    /// Create a number from its memory representation as a byte array in native endianness.
    ///
    /// As the target platform's native endianness is used,
    /// portable code likely wants to use [`from_be_bytes`] or [`from_le_bytes`], as appropriate instead.
    ///
    /// [`from_be_bytes`]: #method.from_be_bytes
    /// [`from_le_bytes`]: #method.from_le_bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::FromBytes;
    ///
    /// #[cfg(target_endian = "big")]
    /// let bytes = [0x12, 0x34, 0x56, 0x78];
    ///
    /// #[cfg(target_endian = "little")]
    /// let bytes = [0x78, 0x56, 0x34, 0x12];
    ///
    /// let value: u32 = FromBytes::from_ne_bytes(&bytes);
    /// assert_eq!(value, 0x12345678)
    /// ```
    fn from_ne_bytes(bytes: &Self::Bytes) -> Self {
        #[cfg(target_endian = "big")]
        let this = Self::from_be_bytes(bytes);
        #[cfg(target_endian = "little")]
        let this = Self::from_le_bytes(bytes);
        this
    }
}

macro_rules! float_to_from_bytes_impl {
    ($T:ty, $L:expr) => {
        impl ToBytes for $T {
            type Bytes = [u8; $L];

            #[inline]
            fn to_be_bytes(&self) -> Self::Bytes {
                <$T>::to_be_bytes(*self)
            }

            #[inline]
            fn to_le_bytes(&self) -> Self::Bytes {
                <$T>::to_le_bytes(*self)
            }

            #[inline]
            fn to_ne_bytes(&self) -> Self::Bytes {
                <$T>::to_ne_bytes(*self)
            }
        }

        impl FromBytes for $T {
            type Bytes = [u8; $L];

            #[inline]
            fn from_be_bytes(bytes: &Self::Bytes) -> Self {
                <$T>::from_be_bytes(*bytes)
            }

            #[inline]
            fn from_le_bytes(bytes: &Self::Bytes) -> Self {
                <$T>::from_le_bytes(*bytes)
            }

            #[inline]
            fn from_ne_bytes(bytes: &Self::Bytes) -> Self {
                <$T>::from_ne_bytes(*bytes)
            }
        }
    };
}

macro_rules! int_to_from_bytes_impl {
    ($T:ty, $L:expr) => {
        impl ToBytes for $T {
            type Bytes = [u8; $L];

            #[inline]
            fn to_be_bytes(&self) -> Self::Bytes {
                <$T>::to_be_bytes(*self)
            }

            #[inline]
            fn to_le_bytes(&self) -> Self::Bytes {
                <$T>::to_le_bytes(*self)
            }

            #[inline]
            fn to_ne_bytes(&self) -> Self::Bytes {
                <$T>::to_ne_bytes(*self)
            }
        }

        impl FromBytes for $T {
            type Bytes = [u8; $L];

            #[inline]
            fn from_be_bytes(bytes: &Self::Bytes) -> Self {
                <$T>::from_be_bytes(*bytes)
            }

            #[inline]
            fn from_le_bytes(bytes: &Self::Bytes) -> Self {
                <$T>::from_le_bytes(*bytes)
            }

            #[inline]
            fn from_ne_bytes(bytes: &Self::Bytes) -> Self {
                <$T>::from_ne_bytes(*bytes)
            }
        }
    };
}

int_to_from_bytes_impl!(u8, 1);
int_to_from_bytes_impl!(u16, 2);
int_to_from_bytes_impl!(u32, 4);
int_to_from_bytes_impl!(u64, 8);
int_to_from_bytes_impl!(u128, 16);
#[cfg(target_pointer_width = "64")]
int_to_from_bytes_impl!(usize, 8);
#[cfg(target_pointer_width = "32")]
int_to_from_bytes_impl!(usize, 4);

int_to_from_bytes_impl!(i8, 1);
int_to_from_bytes_impl!(i16, 2);
int_to_from_bytes_impl!(i32, 4);
int_to_from_bytes_impl!(i64, 8);
int_to_from_bytes_impl!(i128, 16);
#[cfg(target_pointer_width = "64")]
int_to_from_bytes_impl!(isize, 8);
#[cfg(target_pointer_width = "32")]
int_to_from_bytes_impl!(isize, 4);

float_to_from_bytes_impl!(f32, 4);
float_to_from_bytes_impl!(f64, 8);

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! check_to_from_bytes {
        ($( $ty:ty )+) => {$({
            let n = 1;
            let be = <$ty as ToBytes>::to_be_bytes(&n);
            let le = <$ty as ToBytes>::to_le_bytes(&n);
            let ne = <$ty as ToBytes>::to_ne_bytes(&n);

            assert_eq!(*be.last().unwrap(), 1);
            assert_eq!(*le.first().unwrap(), 1);
            if cfg!(target_endian = "big") {
                assert_eq!(*ne.last().unwrap(), 1);
            } else {
                assert_eq!(*ne.first().unwrap(), 1);
            }

            assert_eq!(<$ty as FromBytes>::from_be_bytes(&be), n);
            assert_eq!(<$ty as FromBytes>::from_le_bytes(&le), n);
            if cfg!(target_endian = "big") {
                assert_eq!(<$ty as FromBytes>::from_ne_bytes(&be), n);
            } else {
                assert_eq!(<$ty as FromBytes>::from_ne_bytes(&le), n);
            }
        })+}
    }

    #[test]
    fn convert_between_int_and_bytes() {
        check_to_from_bytes!(u8 u16 u32 u64 u128 usize);
        check_to_from_bytes!(i8 i16 i32 i64 i128 isize);
    }

    #[test]
    fn convert_between_float_and_bytes() {
        macro_rules! check_to_from_bytes {
            ($( $ty:ty )+) => {$(
                let n: $ty = 3.14;

                let be = <$ty as ToBytes>::to_be_bytes(&n);
                let le = <$ty as ToBytes>::to_le_bytes(&n);
                let ne = <$ty as ToBytes>::to_ne_bytes(&n);

                assert_eq!(<$ty as FromBytes>::from_be_bytes(&be), n);
                assert_eq!(<$ty as FromBytes>::from_le_bytes(&le), n);
                if cfg!(target_endian = "big") {
                    assert_eq!(ne, be);
                    assert_eq!(<$ty as FromBytes>::from_ne_bytes(&be), n);
                } else {
                    assert_eq!(ne, le);
                    assert_eq!(<$ty as FromBytes>::from_ne_bytes(&le), n);
                }
            )+}
        }

        check_to_from_bytes!(f32 f64);
    }
}
