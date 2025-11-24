use std::io::{Read, Result, Write};

use crate::Endian;

macro_rules! decl_read_method {
    ($type:ty, $method:ident) => {
        #[doc = concat!("Read a `", stringify!($type), "`.")]
        fn $method(&mut self, endian: Endian) -> Result<$type>;
    };
}

/// A trait for reading bytes.
///
/// This is implemented for all types that implement [`Read`].
pub trait ReadBytes {
    decl_read_method!(u8, read_u8);
    decl_read_method!(u16, read_u16);
    decl_read_method!(u32, read_u32);
    decl_read_method!(u64, read_u64);
    decl_read_method!(u128, read_u128);

    decl_read_method!(i8, read_i8);
    decl_read_method!(i16, read_i16);
    decl_read_method!(i32, read_i32);
    decl_read_method!(i64, read_i64);
    decl_read_method!(i128, read_i128);

    decl_read_method!(f32, read_f32);
    decl_read_method!(f64, read_f64);
}

macro_rules! impl_read_method {
    ($type:ty, $method:ident, $size:literal) => {
        #[inline]
        fn $method(&mut self, endian: Endian) -> Result<$type> {
            let mut buf = [0; $size];
            self.read_exact(&mut buf)?;
            Ok(endian.$method(&buf))
        }
    };
}

impl<R: Read> ReadBytes for R {
    impl_read_method!(u8, read_u8, 1);
    impl_read_method!(u16, read_u16, 2);
    impl_read_method!(u32, read_u32, 4);
    impl_read_method!(u64, read_u64, 8);
    impl_read_method!(u128, read_u128, 16);

    impl_read_method!(i8, read_i8, 1);
    impl_read_method!(i16, read_i16, 2);
    impl_read_method!(i32, read_i32, 4);
    impl_read_method!(i64, read_i64, 8);
    impl_read_method!(i128, read_i128, 16);

    impl_read_method!(f32, read_f32, 4);
    impl_read_method!(f64, read_f64, 8);
}

macro_rules! decl_write_method {
    ($type:ty, $method:ident) => {
        #[doc = concat!("Write a `", stringify!($type), "`.")]
        fn $method(&mut self, endian: Endian, n: $type) -> Result<()>;
    };
}

/// A trait for writing bytes.
///
/// This is implemented for all types that implement [`Write`].
pub trait WriteBytes {
    decl_write_method!(u8, write_u8);
    decl_write_method!(u16, write_u16);
    decl_write_method!(u32, write_u32);
    decl_write_method!(u64, write_u64);
    decl_write_method!(u128, write_u128);

    decl_write_method!(i8, write_i8);
    decl_write_method!(i16, write_i16);
    decl_write_method!(i32, write_i32);
    decl_write_method!(i64, write_i64);
    decl_write_method!(i128, write_i128);

    decl_write_method!(f32, write_f32);
    decl_write_method!(f64, write_f64);
}

macro_rules! impl_write_method {
    ($type:ty, $method:ident, $size:literal) => {
        #[inline]
        fn $method(&mut self, endian: Endian, n: $type) -> Result<()> {
            let mut buf = [0; $size];
            endian.$method(&mut buf, n);
            self.write_all(&buf)
        }
    };
}

impl<W: Write> WriteBytes for W {
    impl_write_method!(u8, write_u8, 1);
    impl_write_method!(u16, write_u16, 2);
    impl_write_method!(u32, write_u32, 4);
    impl_write_method!(u64, write_u64, 8);
    impl_write_method!(u128, write_u128, 16);

    impl_write_method!(i8, write_i8, 1);
    impl_write_method!(i16, write_i16, 2);
    impl_write_method!(i32, write_i32, 4);
    impl_write_method!(i64, write_i64, 8);
    impl_write_method!(i128, write_i128, 16);

    impl_write_method!(f32, write_f32, 4);
    impl_write_method!(f64, write_f64, 8);
}
