use super::*;

use snapbox::prelude::*;
use snapbox::str;

use crate::prelude::*;

mod complete {
    use super::*;

    #[test]
    fn i8_tests() {
        assert_parse!(
            i8.parse_peek(&[0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            i8.parse_peek(&[0x7f][..]),
            str![[r#"
Ok(
    (
        [],
        127,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            i8.parse_peek(&[0xff][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            i8.parse_peek(&[0x80][..]),
            str![[r#"
Ok(
    (
        [],
        -128,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_i8_tests() {
        assert_parse!(
            be_i8.parse_peek(&[0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i8.parse_peek(&[0x7f][..]),
            str![[r#"
Ok(
    (
        [],
        127,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i8.parse_peek(&[0xff][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i8.parse_peek(&[0x80][..]),
            str![[r#"
Ok(
    (
        [],
        -128,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_i16_tests() {
        assert_parse!(
            be_i16.parse_peek(&[0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i16.parse_peek(&[0x7f, 0xff][..]),
            str![[r#"
Ok(
    (
        [],
        32767,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i16.parse_peek(&[0xff, 0xff][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i16.parse_peek(&[0x80, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        -32768,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_u24_tests() {
        assert_parse!(
            be_u24.parse_peek(&[0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_u24.parse_peek(&[0x00, 0xFF, 0xFF][..]),
            str![[r#"
Ok(
    (
        [],
        65535,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_u24.parse_peek(&[0x12, 0x34, 0x56][..]),
            str![[r#"
Ok(
    (
        [],
        1193046,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_i24_tests() {
        assert_parse!(
            be_i24.parse_peek(&[0xFF, 0xFF, 0xFF][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i24.parse_peek(&[0xFF, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        -65536,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i24.parse_peek(&[0xED, 0xCB, 0xAA][..]),
            str![[r#"
Ok(
    (
        [],
        -1193046,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_i32_tests() {
        assert_parse!(
            be_i32.parse_peek(&[0x00, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(&[0x7f, 0xff, 0xff, 0xff][..]),
            str![[r#"
Ok(
    (
        [],
        2147483647,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(&[0xff, 0xff, 0xff, 0xff][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(&[0x80, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        -2147483648,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_i64_tests() {
        assert_parse!(
            be_i64.parse_peek(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(&[0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]),
            str![[r#"
Ok(
    (
        [],
        9223372036854775807,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(&[0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        -9223372036854775808,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_i128_tests() {
        assert_parse!(
            be_i128.parse_peek(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00
                ][..]
            ),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(
                &[
                    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff
                ][..]
            ),
            str![[r#"
Ok(
    (
        [],
        170141183460469231731687303715884105727,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff
                ][..]
            ),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(
                &[
                    0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00
                ][..]
            ),
            str![[r#"
Ok(
    (
        [],
        -170141183460469231731687303715884105728,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i8_tests() {
        assert_parse!(
            le_i8.parse_peek(&[0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i8.parse_peek(&[0x7f][..]),
            str![[r#"
Ok(
    (
        [],
        127,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i8.parse_peek(&[0xff][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i8.parse_peek(&[0x80][..]),
            str![[r#"
Ok(
    (
        [],
        -128,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i16_tests() {
        assert_parse!(
            le_i16.parse_peek(&[0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i16.parse_peek(&[0xff, 0x7f][..]),
            str![[r#"
Ok(
    (
        [],
        32767,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i16.parse_peek(&[0xff, 0xff][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i16.parse_peek(&[0x00, 0x80][..]),
            str![[r#"
Ok(
    (
        [],
        -32768,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_u24_tests() {
        assert_parse!(
            le_u24.parse_peek(&[0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_u24.parse_peek(&[0xFF, 0xFF, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        65535,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_u24.parse_peek(&[0x56, 0x34, 0x12][..]),
            str![[r#"
Ok(
    (
        [],
        1193046,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i24_tests() {
        assert_parse!(
            le_i24.parse_peek(&[0xFF, 0xFF, 0xFF][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i24.parse_peek(&[0x00, 0x00, 0xFF][..]),
            str![[r#"
Ok(
    (
        [],
        -65536,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i24.parse_peek(&[0xAA, 0xCB, 0xED][..]),
            str![[r#"
Ok(
    (
        [],
        -1193046,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i32_tests() {
        assert_parse!(
            le_i32.parse_peek(&[0x00, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i32.parse_peek(&[0xff, 0xff, 0xff, 0x7f][..]),
            str![[r#"
Ok(
    (
        [],
        2147483647,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i32.parse_peek(&[0xff, 0xff, 0xff, 0xff][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i32.parse_peek(&[0x00, 0x00, 0x00, 0x80][..]),
            str![[r#"
Ok(
    (
        [],
        -2147483648,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i64_tests() {
        assert_parse!(
            le_i64.parse_peek(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i64.parse_peek(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f][..]),
            str![[r#"
Ok(
    (
        [],
        9223372036854775807,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i64.parse_peek(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i64.parse_peek(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80][..]),
            str![[r#"
Ok(
    (
        [],
        -9223372036854775808,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i128_tests() {
        assert_parse!(
            le_i128.parse_peek(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00
                ][..]
            ),
            str![[r#"
Ok(
    (
        [],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i128.parse_peek(
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0x7f
                ][..]
            ),
            str![[r#"
Ok(
    (
        [],
        170141183460469231731687303715884105727,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i128.parse_peek(
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff
                ][..]
            ),
            str![[r#"
Ok(
    (
        [],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i128.parse_peek(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x80
                ][..]
            ),
            str![[r#"
Ok(
    (
        [],
        -170141183460469231731687303715884105728,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_f32_tests() {
        assert_parse!(
            be_f32.parse_peek(&[0x00, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0.0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_f32.parse_peek(&[0x4d, 0x31, 0x1f, 0xd8][..]),
            str![[r#"
Ok(
    (
        [],
        185728380.0,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_f64_tests() {
        assert_parse!(
            be_f64.parse_peek(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0.0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_f64.parse_peek(&[0x41, 0xa6, 0x23, 0xfb, 0x10, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        185728392.0,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_f32_tests() {
        assert_parse!(
            le_f32.parse_peek(&[0x00, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0.0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_f32.parse_peek(&[0xd8, 0x1f, 0x31, 0x4d][..]),
            str![[r#"
Ok(
    (
        [],
        185728380.0,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_f64_tests() {
        assert_parse!(
            le_f64.parse_peek(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
            str![[r#"
Ok(
    (
        [],
        0.0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_f64.parse_peek(&[0x00, 0x00, 0x00, 0x10, 0xfb, 0x23, 0xa6, 0x41][..]),
            str![[r#"
Ok(
    (
        [],
        185728392.0,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn configurable_endianness() {
        use crate::binary::Endianness;

        fn be_tst16<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], u16> {
            u16(Endianness::Big).parse_next(i)
        }
        fn le_tst16<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], u16> {
            u16(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tst16.parse_peek(&[0x80, 0x00]),
            str![[r#"
Ok(
    (
        [],
        32768,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tst16.parse_peek(&[0x80, 0x00]),
            str![[r#"
Ok(
    (
        [],
        128,
    ),
)

"#]]
            .raw()
        );

        fn be_tst32<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], u32> {
            u32(Endianness::Big).parse_next(i)
        }
        fn le_tst32<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], u32> {
            u32(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tst32.parse_peek(&[0x12, 0x00, 0x60, 0x00]),
            str![[r#"
Ok(
    (
        [],
        302014464,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tst32.parse_peek(&[0x12, 0x00, 0x60, 0x00]),
            str![[r#"
Ok(
    (
        [],
        6291474,
    ),
)

"#]]
            .raw()
        );

        fn be_tst64<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], u64> {
            u64(Endianness::Big).parse_next(i)
        }
        fn le_tst64<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], u64> {
            u64(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tst64.parse_peek(&[0x12, 0x00, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00]),
            str![[r#"
Ok(
    (
        [],
        1297142246100992000,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tst64.parse_peek(&[0x12, 0x00, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00]),
            str![[r#"
Ok(
    (
        [],
        36028874334666770,
    ),
)

"#]]
            .raw()
        );

        fn be_tsti16<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], i16> {
            i16(Endianness::Big).parse_next(i)
        }
        fn le_tsti16<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], i16> {
            i16(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tsti16.parse_peek(&[0x00, 0x80]),
            str![[r#"
Ok(
    (
        [],
        128,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tsti16.parse_peek(&[0x00, 0x80]),
            str![[r#"
Ok(
    (
        [],
        -32768,
    ),
)

"#]]
            .raw()
        );

        fn be_tsti32<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], i32> {
            i32(Endianness::Big).parse_next(i)
        }
        fn le_tsti32<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], i32> {
            i32(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tsti32.parse_peek(&[0x00, 0x12, 0x60, 0x00]),
            str![[r#"
Ok(
    (
        [],
        1204224,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tsti32.parse_peek(&[0x00, 0x12, 0x60, 0x00]),
            str![[r#"
Ok(
    (
        [],
        6296064,
    ),
)

"#]]
            .raw()
        );

        fn be_tsti64<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], i64> {
            i64(Endianness::Big).parse_next(i)
        }
        fn le_tsti64<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], i64> {
            i64(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tsti64.parse_peek(&[0x00, 0xFF, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00]),
            str![[r#"
Ok(
    (
        [],
        71881672479506432,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tsti64.parse_peek(&[0x00, 0xFF, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00]),
            str![[r#"
Ok(
    (
        [],
        36028874334732032,
    ),
)

"#]]
            .raw()
        );
    }
}

mod partial {
    use super::*;

    #[cfg(feature = "alloc")]
    use crate::lib::std::vec::Vec;
    use crate::Partial;
    use crate::{
        ascii::digit1 as digit,
        binary::{be_u16, be_u8},
        lib::std::str::{self, FromStr},
    };

    #[test]
    fn i8_tests() {
        assert_parse!(
            be_i8.parse_peek(Partial::new(&[0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i8.parse_peek(Partial::new(&[0x7f][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        127,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i8.parse_peek(Partial::new(&[0xff][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i8.parse_peek(Partial::new(&[0x80][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -128,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i8.parse_peek(Partial::new(&[][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn i16_tests() {
        assert_parse!(
            be_i16.parse_peek(Partial::new(&[0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i16.parse_peek(Partial::new(&[0x7f, 0xff][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        32767,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i16.parse_peek(Partial::new(&[0xff, 0xff][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i16.parse_peek(Partial::new(&[0x80, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -32768,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i16.parse_peek(Partial::new(&[][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            2,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i16.parse_peek(Partial::new(&[0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn u24_tests() {
        assert_parse!(
            be_u24.parse_peek(Partial::new(&[0x00, 0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_u24.parse_peek(Partial::new(&[0x00, 0xFF, 0xFF][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        65535,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_u24.parse_peek(Partial::new(&[0x12, 0x34, 0x56][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        1193046,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_u24.parse_peek(Partial::new(&[][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            3,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_u24.parse_peek(Partial::new(&[0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            2,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_u24.parse_peek(Partial::new(&[0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn i24_tests() {
        assert_parse!(
            be_i24.parse_peek(Partial::new(&[0xFF, 0xFF, 0xFF][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i24.parse_peek(Partial::new(&[0xFF, 0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -65536,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i24.parse_peek(Partial::new(&[0xED, 0xCB, 0xAA][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1193046,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i24.parse_peek(Partial::new(&[][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            3,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i24.parse_peek(Partial::new(&[0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            2,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i24.parse_peek(Partial::new(&[0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn i32_tests() {
        assert_parse!(
            be_i32.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(Partial::new(&[0x7f, 0xff, 0xff, 0xff][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        2147483647,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(Partial::new(&[0xff, 0xff, 0xff, 0xff][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(Partial::new(&[0x80, 0x00, 0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -2147483648,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(Partial::new(&[][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            4,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(Partial::new(&[0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            3,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(Partial::new(&[0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            2,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i32.parse_peek(Partial::new(&[0x00, 0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn i64_tests() {
        assert_parse!(
            be_i64.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(
                &[0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        9223372036854775807,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(
                &[0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -9223372036854775808,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(&[][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            8,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(&[0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            7,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(&[0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            6,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(&[0x00, 0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            5,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            4,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            3,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            2,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i64.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn i128_tests() {
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00
                ][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[
                    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff
                ][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        170141183460469231731687303715884105727,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff
                ][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[
                    0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00
                ][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -170141183460469231731687303715884105728,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(&[][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            16,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(&[0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            15,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(&[0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            14,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(&[0x00, 0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            13,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            12,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            11,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            10,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            9,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            8,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            7,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            6,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            5,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            4,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            3,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00
                ][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            2,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_i128.parse_peek(Partial::new(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00
                ][..]
            )),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_u16_tests() {
        assert_parse!(
            le_u16.parse_peek(Partial::new(&[0x00, 0x03][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        768,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_u16.parse_peek(Partial::new(&[b'a', b'b'][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        25185,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_u16.parse_peek(Partial::new(&[0x01][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i8_tests() {
        assert_parse!(
            le_i8.parse_peek(Partial::new(&[0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i8.parse_peek(Partial::new(&[0x7f][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        127,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i8.parse_peek(Partial::new(&[0xff][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i8.parse_peek(Partial::new(&[0x80][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -128,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i16_tests() {
        assert_parse!(
            le_i16.parse_peek(Partial::new(&[0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i16.parse_peek(Partial::new(&[0xff, 0x7f][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        32767,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i16.parse_peek(Partial::new(&[0xff, 0xff][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i16.parse_peek(Partial::new(&[0x00, 0x80][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -32768,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_u24_tests() {
        assert_parse!(
            le_u24.parse_peek(Partial::new(&[0x00, 0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_u24.parse_peek(Partial::new(&[0xFF, 0xFF, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        65535,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_u24.parse_peek(Partial::new(&[0x56, 0x34, 0x12][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        1193046,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i24_tests() {
        assert_parse!(
            le_i24.parse_peek(Partial::new(&[0xFF, 0xFF, 0xFF][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i24.parse_peek(Partial::new(&[0x00, 0x00, 0xFF][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -65536,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i24.parse_peek(Partial::new(&[0xAA, 0xCB, 0xED][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1193046,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_u32_test() {
        assert_parse!(
            le_u32.parse_peek(Partial::new(&[0x00, 0x03, 0x05, 0x07][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        117768960,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_u32.parse_peek(Partial::new(&[b'a', b'b', b'c', b'd'][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        1684234849,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_u32.parse_peek(Partial::new(&[0x01][..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            3,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i32_tests() {
        assert_parse!(
            le_i32.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i32.parse_peek(Partial::new(&[0xff, 0xff, 0xff, 0x7f][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        2147483647,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i32.parse_peek(Partial::new(&[0xff, 0xff, 0xff, 0xff][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i32.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x80][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -2147483648,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i64_tests() {
        assert_parse!(
            le_i64.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i64.parse_peek(Partial::new(
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        9223372036854775807,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i64.parse_peek(Partial::new(
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i64.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -9223372036854775808,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_i128_tests() {
        assert_parse!(
            le_i128.parse_peek(Partial::new(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00
                ][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i128.parse_peek(Partial::new(
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0x7f
                ][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        170141183460469231731687303715884105727,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i128.parse_peek(Partial::new(
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff
                ][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_i128.parse_peek(Partial::new(
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x80
                ][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -170141183460469231731687303715884105728,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_f32_tests() {
        assert_parse!(
            be_f32.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0.0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_f32.parse_peek(Partial::new(&[0x4d, 0x31, 0x1f, 0xd8][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        185728380.0,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn be_f64_tests() {
        assert_parse!(
            be_f64.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0.0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            be_f64.parse_peek(Partial::new(
                &[0x41, 0xa6, 0x23, 0xfb, 0x10, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        185728392.0,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_f32_tests() {
        assert_parse!(
            le_f32.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0.0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_f32.parse_peek(Partial::new(&[0xd8, 0x1f, 0x31, 0x4d][..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        185728380.0,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn le_f64_tests() {
        assert_parse!(
            le_f64.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        0.0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_f64.parse_peek(Partial::new(
                &[0x00, 0x00, 0x00, 0x10, 0xfb, 0x23, 0xa6, 0x41][..]
            )),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        185728392.0,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn configurable_endianness() {
        use crate::binary::Endianness;

        fn be_tst16<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u16> {
            u16(Endianness::Big).parse_next(i)
        }
        fn le_tst16<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u16> {
            u16(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tst16.parse_peek(Partial::new(&[0x80, 0x00])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        32768,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tst16.parse_peek(Partial::new(&[0x80, 0x00])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        128,
    ),
)

"#]]
            .raw()
        );

        fn be_tst32<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u32> {
            u32(Endianness::Big).parse_next(i)
        }
        fn le_tst32<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u32> {
            u32(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tst32.parse_peek(Partial::new(&[0x12, 0x00, 0x60, 0x00])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        302014464,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tst32.parse_peek(Partial::new(&[0x12, 0x00, 0x60, 0x00])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        6291474,
    ),
)

"#]]
            .raw()
        );

        fn be_tst64<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u64> {
            u64(Endianness::Big).parse_next(i)
        }
        fn le_tst64<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u64> {
            u64(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tst64.parse_peek(Partial::new(&[
                0x12, 0x00, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00
            ])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        1297142246100992000,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tst64.parse_peek(Partial::new(&[
                0x12, 0x00, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00
            ])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        36028874334666770,
    ),
)

"#]]
            .raw()
        );

        fn be_tsti16<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, i16> {
            i16(Endianness::Big).parse_next(i)
        }
        fn le_tsti16<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, i16> {
            i16(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tsti16.parse_peek(Partial::new(&[0x00, 0x80])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        128,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tsti16.parse_peek(Partial::new(&[0x00, 0x80])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        -32768,
    ),
)

"#]]
            .raw()
        );

        fn be_tsti32<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, i32> {
            i32(Endianness::Big).parse_next(i)
        }
        fn le_tsti32<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, i32> {
            i32(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tsti32.parse_peek(Partial::new(&[0x00, 0x12, 0x60, 0x00])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        1204224,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tsti32.parse_peek(Partial::new(&[0x00, 0x12, 0x60, 0x00])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        6296064,
    ),
)

"#]]
            .raw()
        );

        fn be_tsti64<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, i64> {
            i64(Endianness::Big).parse_next(i)
        }
        fn le_tsti64<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, i64> {
            i64(Endianness::Little).parse_next(i)
        }
        assert_parse!(
            be_tsti64.parse_peek(Partial::new(&[
                0x00, 0xFF, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00
            ])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        71881672479506432,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            le_tsti64.parse_peek(Partial::new(&[
                0x00, 0xFF, 0x60, 0x00, 0x12, 0x00, 0x80, 0x00
            ])),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        36028874334732032,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn length_repeat_test() {
        fn number<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u32> {
            digit
                .try_map(str::from_utf8)
                .try_map(FromStr::from_str)
                .parse_next(i)
        }

        fn cnt<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
            length_repeat(number, "abc").parse_next(i)
        }

        assert_parse!(
            cnt.parse_peek(Partial::new(&b"2abcabcabcdef"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                98,
                99,
                100,
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
            ],
            [
                97,
                98,
                99,
            ],
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            cnt.parse_peek(Partial::new(&b"2ab"[..])),
            str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            cnt.parse_peek(Partial::new(&b"3abcab"[..])),
            str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            cnt.parse_peek(Partial::new(&b"xxx"[..])),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            cnt.parse_peek(Partial::new(&b"2abcxxx"[..])),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn partial_length_bytes() {
        use crate::binary::le_u8;

        fn x<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
            length_take(le_u8).parse_next(i)
        }
        assert_parse!(
            x.parse_peek(Partial::new(b"\x02..>>")),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                62,
                62,
            ],
            partial: true,
        },
        [
            46,
            46,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            x.parse_peek(Partial::new(b"\x02..")),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        [
            46,
            46,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            x.parse_peek(Partial::new(b"\x02.")),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            x.parse_peek(Partial::new(b"\x02")),
            str![[r#"
Err(
    Incomplete(
        Size(
            2,
        ),
    ),
)

"#]]
            .raw()
        );

        fn y<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
            let _ = "magic".parse_next(i)?;
            length_take(le_u8).parse_next(i)
        }
        assert_parse!(
            y.parse_peek(Partial::new(b"magic\x02..>>")),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                62,
                62,
            ],
            partial: true,
        },
        [
            46,
            46,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            y.parse_peek(Partial::new(b"magic\x02..")),
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        [
            46,
            46,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            y.parse_peek(Partial::new(b"magic\x02.")),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            y.parse_peek(Partial::new(b"magic\x02")),
            str![[r#"
Err(
    Incomplete(
        Size(
            2,
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn length_take_test() {
        fn number<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u32> {
            digit
                .try_map(str::from_utf8)
                .try_map(FromStr::from_str)
                .parse_next(i)
        }

        fn take<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
            length_take(number).parse_next(i)
        }

        assert_parse!(
            take.parse_peek(Partial::new(&b"6abcabcabcdef"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                98,
                99,
                100,
                101,
                102,
            ],
            partial: true,
        },
        [
            97,
            98,
            99,
            97,
            98,
            99,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            take.parse_peek(Partial::new(&b"3ab"[..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            take.parse_peek(Partial::new(&b"xxx"[..])),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            take.parse_peek(Partial::new(&b"2abcxxx"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                99,
                120,
                120,
                120,
            ],
            partial: true,
        },
        [
            97,
            98,
        ],
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn length_and_then_test() {
        use crate::stream::StreamIsPartial;

        fn length_and_then_1<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u16> {
            length_and_then(be_u8, be_u16).parse_next(i)
        }
        fn length_and_then_2<'i>(
            i: &mut Partial<&'i [u8]>,
        ) -> TestResult<Partial<&'i [u8]>, (u8, u8)> {
            length_and_then(be_u8, (be_u8, be_u8)).parse_next(i)
        }

        let mut empty_complete = Partial::new(&b""[..]);
        let _ = empty_complete.complete();

        let i1 = [0, 5, 6];
        assert_parse!(
            length_and_then_1.parse_peek(Partial::new(&i1)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [],
                partial: false,
            },
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            length_and_then_2.parse_peek(Partial::new(&i1)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [],
                partial: false,
            },
        },
    ),
)

"#]]
            .raw()
        );

        let i2 = [1, 5, 6, 3];
        {
            let mut middle_complete = Partial::new(&i2[1..2]);
            let _ = middle_complete.complete();
            assert_parse!(
                length_and_then_1.parse_peek(Partial::new(&i2)),
                str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    5,
                ],
                partial: false,
            },
        },
    ),
)

"#]]
                .raw()
            );
            assert_parse!(
                length_and_then_2.parse_peek(Partial::new(&i2)),
                str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [],
                partial: false,
            },
        },
    ),
)

"#]]
                .raw()
            );
        }

        let i3 = [2, 5, 6, 3, 4, 5, 7];
        assert_parse!(
            length_and_then_1.parse_peek(Partial::new(&i3)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                3,
                4,
                5,
                7,
            ],
            partial: true,
        },
        1286,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            length_and_then_2.parse_peek(Partial::new(&i3)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                3,
                4,
                5,
                7,
            ],
            partial: true,
        },
        (
            5,
            6,
        ),
    ),
)

"#]]
            .raw()
        );

        let i4 = [3, 5, 6, 3, 4, 5];
        assert_parse!(
            length_and_then_1.parse_peek(Partial::new(&i4)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                4,
                5,
            ],
            partial: true,
        },
        1286,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            length_and_then_2.parse_peek(Partial::new(&i4)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                4,
                5,
            ],
            partial: true,
        },
        (
            5,
            6,
        ),
    ),
)

"#]]
            .raw()
        );
    }
}
