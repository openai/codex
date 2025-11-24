/// The endian of the data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endian {
    /// Little endian.
    Little,
    /// Big endian.
    Big,
}

/// Alias for [`Endian::Little`].
pub const LE: Endian = Endian::Little;
/// Alias for [`Endian::Big`].
pub const BE: Endian = Endian::Big;
/// Same as the return value of [`Endian::native`].
pub const NATIVE_ENDIAN: Endian = Endian::native();
/// Alias for [`Endian::Big`].
pub const NETWORK_ENDIAN: Endian = Endian::Big;

macro_rules! impl_read_method {
    ($type:ty, $method:ident, $size:literal) => {
        #[doc = concat!("Read a `", stringify!($type), "` from a byte slice.\n\n", "# Panics\n\n", "Panics if the slice is smaller than ", stringify!($size), " bytes.")]
        #[inline]
        pub fn $method(self, buf: &[u8]) -> $type {
            match self {
                Self::Little => <$type>::from_le_bytes(buf[..$size].try_into().unwrap()),
                Self::Big => <$type>::from_be_bytes(buf[..$size].try_into().unwrap()),
            }
        }
    };
}

macro_rules! impl_write_method {
    ($type:ty, $method:ident, $size:literal) => {
        #[doc = concat!("Write a `", stringify!($type), "` into a mutable byte slice.\n\n", "# Panics\n\n", "Panics if the slice is smaller than ", stringify!($size), " bytes.")]
        #[inline]
        pub fn $method(self, buf: &mut [u8], n: $type) {
            match self {
                Self::Little => buf[..$size].copy_from_slice(&n.to_le_bytes()),
                Self::Big => buf[..$size].copy_from_slice(&n.to_be_bytes()),
            }
        }
    };
}

impl Endian {
    /// The native endian.
    #[inline]
    pub const fn native() -> Self {
        #[cfg(target_endian = "little")]
        {
            Self::Little
        }
        #[cfg(target_endian = "big")]
        {
            Self::Big
        }
    }

    // Reading.

    // Unsigned integers
    impl_read_method!(u8, read_u8, 1);
    impl_read_method!(u16, read_u16, 2);
    impl_read_method!(u32, read_u32, 4);
    impl_read_method!(u64, read_u64, 8);
    impl_read_method!(u128, read_u128, 16);

    // Signed integers
    impl_read_method!(i8, read_i8, 1);
    impl_read_method!(i16, read_i16, 2);
    impl_read_method!(i32, read_i32, 4);
    impl_read_method!(i64, read_i64, 8);
    impl_read_method!(i128, read_i128, 16);

    // Floating point numbers
    impl_read_method!(f32, read_f32, 4);
    impl_read_method!(f64, read_f64, 8);

    // Writing.

    // Unsigned integers
    impl_write_method!(u8, write_u8, 1);
    impl_write_method!(u16, write_u16, 2);
    impl_write_method!(u32, write_u32, 4);
    impl_write_method!(u64, write_u64, 8);
    impl_write_method!(u128, write_u128, 16);

    // Signed integers
    impl_write_method!(i8, write_i8, 1);
    impl_write_method!(i16, write_i16, 2);
    impl_write_method!(i32, write_i32, 4);
    impl_write_method!(i64, write_i64, 8);
    impl_write_method!(i128, write_i128, 16);

    // Floating point numbers
    impl_write_method!(f32, write_f32, 4);
    impl_write_method!(f64, write_f64, 8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u8() {
        let buf = [0x01];
        assert_eq!(Endian::Little.read_u8(&buf), 0x01);
        assert_eq!(Endian::Big.read_u8(&buf), 0x01);
        let mut buf = [0x00];
        Endian::Little.write_u8(&mut buf, 0x01);
        assert_eq!(buf, [0x01]);
        Endian::Big.write_u8(&mut buf, 0x01);
        assert_eq!(buf, [0x01]);
    }

    #[test]
    fn u16() {
        let buf = [0x01, 0x02];
        assert_eq!(Endian::Little.read_u16(&buf), 0x02_01);
        assert_eq!(Endian::Big.read_u16(&buf), 0x01_02);
        let mut buf = [0x00, 0x00];
        Endian::Little.write_u16(&mut buf, 0x01_02);
        assert_eq!(buf, [0x02, 0x01]);
        Endian::Big.write_u16(&mut buf, 0x01_02);
        assert_eq!(buf, [0x01, 0x02]);
    }

    #[test]
    fn u32() {
        let buf = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(Endian::Little.read_u32(&buf), 0x04_03_02_01);
        assert_eq!(Endian::Big.read_u32(&buf), 0x01_02_03_04);
        let mut buf = [0x00, 0x00, 0x00, 0x00];
        Endian::Little.write_u32(&mut buf, 0x01_02_03_04);
        assert_eq!(buf, [0x04, 0x03, 0x02, 0x01]);
        Endian::Big.write_u32(&mut buf, 0x01_02_03_04);
        assert_eq!(buf, [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn u64() {
        let buf = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        assert_eq!(Endian::Little.read_u64(&buf), 0x08_07_06_05_04_03_02_01);
        assert_eq!(Endian::Big.read_u64(&buf), 0x01_02_03_04_05_06_07_08);
        let mut buf = [0x00; 8];
        Endian::Little.write_u64(&mut buf, 0x01_02_03_04_05_06_07_08);
        assert_eq!(buf, [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
        Endian::Big.write_u64(&mut buf, 0x01_02_03_04_05_06_07_08);
        assert_eq!(buf, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn u128() {
        let buf = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10,
        ];
        assert_eq!(
            Endian::Little.read_u128(&buf),
            0x10_0f_0e_0d_0c_0b_0a_09_08_07_06_05_04_03_02_01
        );
        assert_eq!(
            Endian::Big.read_u128(&buf),
            0x01_02_03_04_05_06_07_08_09_0a_0b_0c_0d_0e_0f_10
        );
        let mut buf = [0x00; 16];
        Endian::Little.write_u128(&mut buf, 0x01_02_03_04_05_06_07_08_09_0a_0b_0c_0d_0e_0f_10);
        assert_eq!(
            buf,
            [
                0x10, 0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03,
                0x02, 0x01
            ]
        );
        Endian::Big.write_u128(&mut buf, 0x01_02_03_04_05_06_07_08_09_0a_0b_0c_0d_0e_0f_10);
        assert_eq!(
            buf,
            [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
                0x0f, 0x10
            ]
        );
    }

    #[test]
    fn i8() {
        let buf = [0x01];
        assert_eq!(Endian::Little.read_i8(&buf), 0x01);
        assert_eq!(Endian::Big.read_i8(&buf), 0x01);
        let mut buf = [0x00];
        Endian::Little.write_i8(&mut buf, 0x01);
        assert_eq!(buf, [0x01]);
        Endian::Big.write_i8(&mut buf, 0x01);
        assert_eq!(buf, [0x01]);
    }

    #[test]
    fn i16() {
        let buf = [0x01, 0x02];
        assert_eq!(Endian::Little.read_i16(&buf), 0x02_01);
        assert_eq!(Endian::Big.read_i16(&buf), 0x01_02);
        let mut buf = [0x00, 0x00];
        Endian::Little.write_i16(&mut buf, 0x01_02);
        assert_eq!(buf, [0x02, 0x01]);
        Endian::Big.write_i16(&mut buf, 0x01_02);
        assert_eq!(buf, [0x01, 0x02]);
    }

    #[test]
    fn i32() {
        let buf = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(Endian::Little.read_i32(&buf), 0x04_03_02_01);
        assert_eq!(Endian::Big.read_i32(&buf), 0x01_02_03_04);
        let mut buf = [0x00, 0x00, 0x00, 0x00];
        Endian::Little.write_i32(&mut buf, 0x01_02_03_04);
        assert_eq!(buf, [0x04, 0x03, 0x02, 0x01]);
        Endian::Big.write_i32(&mut buf, 0x01_02_03_04);
        assert_eq!(buf, [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn i64() {
        let buf = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        assert_eq!(Endian::Little.read_i64(&buf), 0x08_07_06_05_04_03_02_01);
        assert_eq!(Endian::Big.read_i64(&buf), 0x01_02_03_04_05_06_07_08);
        let mut buf = [0x00; 8];
        Endian::Little.write_i64(&mut buf, 0x01_02_03_04_05_06_07_08);
        assert_eq!(buf, [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
        Endian::Big.write_i64(&mut buf, 0x01_02_03_04_05_06_07_08);
        assert_eq!(buf, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn f32() {
        let buf = [0x00, 0x00, 0x80, 0x3f];
        assert_eq!(Endian::Little.read_f32(&buf), 1.0);
        assert_eq!(Endian::Big.read_f32(&buf), 4.6006e-41);
        let mut buf = [0x00; 4];
        Endian::Little.write_f32(&mut buf, 1.0);
        assert_eq!(buf, [0x00, 0x00, 0x80, 0x3f]);
        Endian::Big.write_f32(&mut buf, 1.0);
        assert_eq!(buf, [0x3f, 0x80, 0x00, 0x00]);
    }

    #[test]
    fn f64() {
        let buf = [0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0xbf, 0x3f];
        assert_eq!(Endian::Little.read_f64(&buf), 0.124755859375);
        assert_eq!(Endian::Big.read_f64(&buf), 7.7951696e-317);
        let mut buf = [0x00; 8];
        Endian::Little.write_f64(&mut buf, 0.124755859375);
        assert_eq!(buf, [0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0xbf, 0x3f]);
        Endian::Big.write_f64(&mut buf, 1.0);
        assert_eq!(buf, [0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    }
}
