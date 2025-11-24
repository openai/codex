#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString};

use derive_more::{
    Binary, Display, LowerExp, LowerHex, Octal, Pointer, UpperExp, UpperHex,
};

mod structs {
    use super::*;

    mod unit {
        use super::*;

        #[derive(Display)]
        struct Unit;

        #[derive(Display)]
        struct r#RawUnit;

        #[derive(Display)]
        struct Tuple();

        #[derive(Display)]
        struct Struct {}

        #[test]
        fn assert() {
            assert_eq!(Unit.to_string(), "Unit");
            assert_eq!(r#RawUnit.to_string(), "RawUnit");
            assert_eq!(Tuple().to_string(), "Tuple");
            assert_eq!(Struct {}.to_string(), "Struct");
        }

        mod str {
            use super::*;

            #[derive(Display)]
            #[display("unit")]
            pub struct Unit;

            #[derive(Display)]
            #[display("tuple")]
            pub struct Tuple();

            #[derive(Display)]
            #[display("struct")]
            pub struct Struct {}

            #[test]
            fn assert() {
                assert_eq!(Unit.to_string(), "unit");
                assert_eq!(Tuple().to_string(), "tuple");
                assert_eq!(Struct {}.to_string(), "struct");
            }
        }

        mod interpolated {
            use super::*;

            #[derive(Display)]
            #[display("unit: {}", 0)]
            pub struct Unit;

            #[derive(Display)]
            #[display("tuple: {}", 0)]
            pub struct Tuple();

            #[derive(Display)]
            #[display("struct: {}", 0)]
            pub struct Struct {}

            #[test]
            fn assert() {
                assert_eq!(Unit.to_string(), "unit: 0");
                assert_eq!(Tuple().to_string(), "tuple: 0");
                assert_eq!(Struct {}.to_string(), "struct: 0");
            }
        }

        mod transparency {
            use super::*;

            mod interpolated {
                use super::*;

                const I32: i32 = 11;
                const F64: f64 = 3.15;
                const POINTER: &f64 = &3.15;

                #[derive(Display)]
                #[display("{I32}")]
                struct Display;

                #[derive(Display)]
                #[display("{I32:?}")]
                struct Debug;

                #[derive(Display)]
                #[display("{:b}", I32)]
                struct Binary;

                #[derive(Display)]
                #[display("{0:o}", I32)]
                struct Octal;

                #[derive(Display)]
                #[display("{I32:x}")]
                struct LowerHex;

                #[derive(Display)]
                #[display("{:X}", I32)]
                struct UpperHex;

                #[derive(Display)]
                #[display("{F64:e}")]
                struct LowerExp;

                #[derive(Display)]
                #[display("{named:E}", named = F64)]
                struct UpperExp;

                #[derive(Display)]
                #[display("{POINTER:p}")]
                struct Pointer;

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", Display), "011");
                    assert_eq!(format!("{:03}", Debug), "011");
                    assert_eq!(format!("{:07}", Binary), "0001011");
                    assert_eq!(format!("{:07}", Octal), "0000013");
                    assert_eq!(format!("{:03}", LowerHex), "00b");
                    assert_eq!(format!("{:03}", UpperHex), "00B");
                    assert_eq!(format!("{:07}", LowerExp), "03.15e0");
                    assert_eq!(format!("{:07}", UpperExp), "03.15E0");
                    assert_eq!(format!("{:018}", Pointer), format!("{POINTER:018p}"));
                }
            }

            mod omitted {
                use super::*;

                mod on_modifiers {
                    use super::*;

                    const I32: i32 = 11;
                    const F64: f64 = 3.15;

                    #[derive(Display)]
                    #[display("{I32:x?}")]
                    struct LowerDebug;

                    #[derive(Display)]
                    #[display("{I32:X?}")]
                    struct UpperDebug;

                    #[derive(Display)]
                    #[display("{:^}", I32)]
                    struct Align;

                    #[derive(Display)]
                    #[display("{:+}", I32)]
                    struct Sign;

                    #[derive(Display)]
                    #[display("{:#b}", I32)]
                    struct Alternate;

                    #[derive(Display)]
                    #[display("{:0}", I32)]
                    struct ZeroPadded;

                    #[derive(Display)]
                    #[display("{:07}", I32)]
                    struct Width;

                    #[derive(Display)]
                    #[display("{:.1}", F64)]
                    struct Precision;

                    #[test]
                    fn assert() {
                        assert_eq!(format!("{:03}", LowerDebug), "b");
                        assert_eq!(format!("{:03}", UpperDebug), "B");
                        assert_eq!(format!("{:03}", Align), "11");
                        assert_eq!(format!("{:04}", Sign), "+11");
                        assert_eq!(format!("{:07}", Alternate), "0b1011");
                        assert_eq!(format!("{:07}", ZeroPadded), "11");
                        assert_eq!(format!("{:03}", Width), "0000011");
                        assert_eq!(format!("{:.3}", Precision), "3.1");
                    }
                }
            }
        }
    }

    mod single_field {
        use super::*;

        #[derive(Display)]
        struct Tuple(i32);

        #[derive(Binary)]
        struct Binary(i32);

        #[derive(Display)]
        struct Struct {
            field: i32,
        }

        #[derive(Octal)]
        struct Octal {
            field: i32,
        }

        #[test]
        fn assert() {
            assert_eq!(Tuple(0).to_string(), "0");
            assert_eq!(format!("{:b}", Binary(10)), "1010");
            assert_eq!(Struct { field: 0 }.to_string(), "0");
            assert_eq!(format!("{:o}", Octal { field: 10 }).to_string(), "12");
        }

        mod str {
            use super::*;

            #[derive(Display)]
            #[display("tuple")]
            struct Tuple(i32);

            #[derive(Display)]
            #[display("struct")]
            struct Struct {
                field: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple(0).to_string(), "tuple");
                assert_eq!(Struct { field: 0 }.to_string(), "struct");
            }
        }

        mod interpolated {
            use super::*;

            #[derive(Display)]
            #[display("tuple: {_0} {}", _0)]
            struct Tuple(i32);

            #[derive(Display)]
            #[display("struct: {field} {}", field)]
            struct Struct {
                field: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple(0).to_string(), "tuple: 0 0");
                assert_eq!(Struct { field: 0 }.to_string(), "struct: 0 0");
            }
        }

        mod transparency {
            use super::*;

            mod direct {
                use super::*;

                #[derive(Display)]
                struct TupleDisplay(i32);

                #[derive(Binary)]
                struct TupleBinary(i32);

                #[derive(Octal)]
                struct TupleOctal(i32);

                #[derive(LowerHex)]
                struct StructLowerHex {
                    field: i32,
                }

                #[derive(UpperHex)]
                struct StructUpperHex {
                    field: i32,
                }

                #[derive(LowerExp)]
                struct StructLowerExp {
                    field: f64,
                }

                #[derive(UpperExp)]
                struct StructUpperExp {
                    field: f64,
                }

                #[derive(Pointer)]
                struct StructPointer<'a> {
                    field: &'a i32,
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", TupleDisplay(7)), "007");
                    assert_eq!(format!("{:07b}", TupleBinary(7)), "0000111");
                    assert_eq!(format!("{:03o}", TupleOctal(9)), "011");
                    assert_eq!(format!("{:03x}", StructLowerHex { field: 42 }), "02a");
                    assert_eq!(format!("{:03X}", StructUpperHex { field: 42 }), "02A");
                    assert_eq!(
                        format!("{:07e}", StructLowerExp { field: 42.0 }),
                        "004.2e1",
                    );
                    assert_eq!(
                        format!("{:07E}", StructUpperExp { field: 42.0 }),
                        "004.2E1",
                    );
                    let a = 42;
                    assert_eq!(
                        format!("{:018p}", StructPointer { field: &a }),
                        format!("{:018p}", &a),
                    );
                }
            }

            mod interpolated {
                use super::*;

                #[derive(Display)]
                #[display("{_0}")]
                struct TupleDisplay(i32);

                #[derive(Display)]
                #[display("{_0:?}")]
                struct TupleDebug(i32);

                #[derive(Display)]
                #[display("{:b}", _0)]
                struct TupleBinary(i32);

                #[derive(Display)]
                #[display("{0:o}", _0)]
                struct TupleOctal(i32);

                #[derive(Display)]
                #[display("{field:x}")]
                struct StructLowerHex {
                    field: i32,
                }

                #[derive(Display)]
                #[display("{:X}", field)]
                struct StructUpperHex {
                    field: i32,
                }

                #[derive(Display)]
                #[display("{field:e}")]
                struct StructLowerExp {
                    field: f64,
                }

                #[derive(Display)]
                #[display("{named:E}", named = field)]
                struct StructUpperExp {
                    field: f64,
                }

                #[derive(Display)]
                #[display("{field:p}")]
                struct StructPointer<'a> {
                    field: &'a i32,
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", TupleDisplay(7)), "007");
                    assert_eq!(format!("{:03}", TupleDebug(8)), "008");
                    assert_eq!(format!("{:07}", TupleBinary(7)), "0000111");
                    assert_eq!(format!("{:03}", TupleOctal(9)), "011");
                    assert_eq!(format!("{:03}", StructLowerHex { field: 42 }), "02a");
                    assert_eq!(format!("{:03}", StructUpperHex { field: 42 }), "02A");
                    assert_eq!(
                        format!("{:07}", StructLowerExp { field: 42.0 }),
                        "004.2e1",
                    );
                    assert_eq!(
                        format!("{:07}", StructUpperExp { field: 42.0 }),
                        "004.2E1",
                    );
                    let a = 42;
                    assert_eq!(
                        format!("{:018}", StructPointer { field: &a }),
                        format!("{:018p}", &a),
                    );
                }
            }

            mod suppressed {
                use super::*;

                #[derive(Display)]
                #[display("{}", format_args!("{_0}"))]
                struct TupleDisplay(i32);

                #[derive(Display)]
                #[display("{}", format_args!("{_0:?}"))]
                struct TupleDebug(i32);

                #[derive(Display)]
                #[display("{}", format_args!("{_0:b}"))]
                struct TupleBinary(i32);

                #[derive(Display)]
                #[display("{}", format_args!("{_0:o}"))]
                struct TupleOctal(i32);

                #[derive(Display)]
                #[display("{}", format_args!("{field:x}"))]
                struct StructLowerHex {
                    field: i32,
                }

                #[derive(Display)]
                #[display("{}", format_args!("{field:X}"))]
                struct StructUpperHex {
                    field: i32,
                }

                #[derive(Display)]
                #[display("{}", format_args!("{field:e}"))]
                struct StructLowerExp {
                    field: f64,
                }

                #[derive(Display)]
                #[display("{}", format_args!("{field:E}"))]
                struct StructUpperExp {
                    field: f64,
                }

                #[derive(Display)]
                #[display("{}", format_args!("{field:p}", field = *field))]
                struct StructPointer<'a> {
                    field: &'a i32,
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", TupleDisplay(7)), "7");
                    assert_eq!(format!("{:03}", TupleDebug(8)), "8");
                    assert_eq!(format!("{:07}", TupleBinary(7)), "111");
                    assert_eq!(format!("{:03}", TupleOctal(9)), "11");
                    assert_eq!(format!("{:03}", StructLowerHex { field: 42 }), "2a");
                    assert_eq!(format!("{:03}", StructUpperHex { field: 42 }), "2A");
                    assert_eq!(
                        format!("{:07}", StructLowerExp { field: 42.0 }),
                        "4.2e1",
                    );
                    assert_eq!(
                        format!("{:07}", StructUpperExp { field: 42.0 }),
                        "4.2E1",
                    );
                    let a = 42;
                    assert_eq!(
                        format!("{:018}", StructPointer { field: &a }),
                        format!("{:p}", &a),
                    );
                }
            }

            mod omitted {
                use super::*;

                mod on_modifiers {
                    use super::*;

                    #[derive(Display)]
                    #[display("{_0:x?}")]
                    struct LowerDebug(i32);

                    #[derive(Display)]
                    #[display("{_0:X?}")]
                    struct UpperDebug(i32);

                    #[derive(Display)]
                    #[display("{:^}", _0)]
                    struct Align(i32);

                    #[derive(Display)]
                    #[display("{:+}", _0)]
                    struct Sign(i32);

                    #[derive(Display)]
                    #[display("{:#b}", _0)]
                    struct Alternate(i32);

                    #[derive(Display)]
                    #[display("{:0}", _0)]
                    struct ZeroPadded(i32);

                    #[derive(Display)]
                    #[display("{:07}", _0)]
                    struct Width(i32);

                    #[derive(Display)]
                    #[display("{:.5}", _0)]
                    struct Precision(f64);

                    #[test]
                    fn assert() {
                        assert_eq!(format!("{:03}", LowerDebug(7)), "7");
                        assert_eq!(format!("{:03}", UpperDebug(8)), "8");
                        assert_eq!(format!("{:03}", Align(5)), "5");
                        assert_eq!(format!("{:03}", Sign(5)), "+5");
                        assert_eq!(format!("{:07}", Alternate(5)), "0b101");
                        assert_eq!(format!("{:07}", ZeroPadded(-5)), "-5");
                        assert_eq!(format!("{:03}", Width(5)), "0000005");
                        assert_eq!(format!("{:.3}", Precision(1.23456789)), "1.23457");
                    }
                }
            }
        }
    }

    mod multi_field {
        use super::*;

        mod str {
            use super::*;

            #[derive(Display)]
            #[display("tuple")]
            struct Tuple(i32, i32);

            #[derive(Display)]
            #[display("struct")]
            struct Struct {
                field1: i32,
                field2: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple(1, 2).to_string(), "tuple");
                assert_eq!(
                    Struct {
                        field1: 1,
                        field2: 2,
                    }
                    .to_string(),
                    "struct",
                );
            }
        }

        mod interpolated {
            use super::*;

            #[derive(Display)]
            #[display(
                "{_0} {ident} {_1} {} {}",
                _1, _0 + _1, ident = 123, _1 = _0,
            )]
            struct Tuple(i32, i32);

            #[derive(Display)]
            #[display(
                "{field1} {ident} {field2} {} {}",
                field2, field1 + field2, ident = 123, field2 = field1,
            )]
            struct Struct {
                field1: i32,
                field2: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple(1, 2).to_string(), "1 123 1 2 3");
                assert_eq!(
                    Struct {
                        field1: 1,
                        field2: 2,
                    }
                    .to_string(),
                    "1 123 1 2 3",
                );
            }
        }

        mod transparency {
            use super::*;

            mod interpolated {
                use super::*;

                #[derive(Display)]
                #[display("{_0}")]
                struct TupleDisplay(i32, u64);

                #[derive(Display)]
                #[display("{:?}", _1)]
                struct TupleDebug(i32, u64);

                #[derive(Display)]
                #[display("{0:b}", _1)]
                struct TupleBinary(i32, u64);

                #[derive(Display)]
                #[display("{named:o}", named = _0)]
                struct TupleOctal(i32, u64);

                #[derive(Display)]
                #[display("{b:x}")]
                struct StructLowerHex {
                    a: i32,
                    b: u64,
                }

                #[derive(Display)]
                #[display("{:X}", a)]
                struct StructUpperHex {
                    a: i32,
                    b: u64,
                }

                #[derive(Display)]
                #[display("{a:e}")]
                struct StructLowerExp {
                    a: f64,
                    b: f32,
                }

                #[derive(Display)]
                #[display("{:E}", b)]
                struct StructUpperExp {
                    a: f64,
                    b: f32,
                }

                #[derive(Display)]
                #[display("{b:p}")]
                struct StructPointer<'a> {
                    a: &'a i32,
                    b: &'a i32,
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", TupleDisplay(7, 8)), "007");
                    assert_eq!(format!("{:03}", TupleDebug(7, 8)), "008");
                    assert_eq!(format!("{:07}", TupleBinary(6, 7)), "0000111");
                    assert_eq!(format!("{:03}", TupleOctal(9, 10)), "011");
                    assert_eq!(
                        format!("{:03}", StructLowerHex { a: 41, b: 42 }),
                        "02a"
                    );
                    assert_eq!(
                        format!("{:03}", StructUpperHex { a: 42, b: 43 }),
                        "02A"
                    );
                    assert_eq!(
                        format!("{:07}", StructLowerExp { a: 42.0, b: 43.0 }),
                        "004.2e1",
                    );
                    assert_eq!(
                        format!("{:07}", StructUpperExp { a: 41.0, b: 42.0 }),
                        "004.2E1",
                    );
                    let (a, b) = (42, 43);
                    assert_eq!(
                        format!("{:018}", StructPointer { a: &a, b: &b }),
                        format!("{:018p}", &b),
                    );
                }
            }
        }
    }
}

mod enums {
    use super::*;

    mod no_variants {
        use super::*;

        #[derive(Display)]
        enum Void {}

        const fn assert<T: Display>() {}
        const _: () = assert::<Void>();
    }

    mod unit_variant {
        use super::*;

        #[derive(Display)]
        enum Enum {
            Unit,
            r#RawUnit,
            Unnamed(),
            Named {},
            #[display("STR_UNIT")]
            StrUnit,
            #[display("STR_UNNAMED")]
            StrUnnamed(),
            #[display("STR_NAMED")]
            StrNamed {},
        }

        #[test]
        fn assert() {
            assert_eq!(Enum::Unit.to_string(), "Unit");
            assert_eq!(Enum::r#RawUnit.to_string(), "RawUnit");
            assert_eq!(Enum::Unnamed().to_string(), "Unnamed");
            assert_eq!(Enum::Named {}.to_string(), "Named");
            assert_eq!(Enum::StrUnit.to_string(), "STR_UNIT");
            assert_eq!(Enum::StrUnnamed().to_string(), "STR_UNNAMED");
            assert_eq!(Enum::StrNamed {}.to_string(), "STR_NAMED");
        }

        mod transparency {
            use super::*;

            mod interpolated {
                use super::*;

                const I32: i32 = 11;
                const F64: f64 = 3.15;
                const POINTER: &f64 = &3.15;

                #[derive(Display)]
                enum Unit {
                    #[display("{I32}")]
                    Display,
                    #[display("{I32:?}")]
                    Debug,
                    #[display("{:b}", I32)]
                    Binary,
                    #[display("{0:o}", I32)]
                    Octal,
                    #[display("{I32:x}")]
                    LowerHex,
                    #[display("{:X}", I32)]
                    UpperHex,
                    #[display("{F64:e}")]
                    LowerExp,
                    #[display("{named:E}", named = F64)]
                    UpperExp,
                    #[display("{POINTER:p}")]
                    Pointer,
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", Unit::Display), "011");
                    assert_eq!(format!("{:03}", Unit::Debug), "011");
                    assert_eq!(format!("{:07}", Unit::Binary), "0001011");
                    assert_eq!(format!("{:07}", Unit::Octal), "0000013");
                    assert_eq!(format!("{:03}", Unit::LowerHex), "00b");
                    assert_eq!(format!("{:03}", Unit::UpperHex), "00B");
                    assert_eq!(format!("{:07}", Unit::LowerExp), "03.15e0");
                    assert_eq!(format!("{:07}", Unit::UpperExp), "03.15E0");
                    assert_eq!(
                        format!("{:018}", Unit::Pointer),
                        format!("{POINTER:018p}"),
                    );
                }
            }

            mod omitted {
                use super::*;

                mod on_modifiers {
                    use super::*;

                    const I32: i32 = 11;
                    const F64: f64 = 3.15;

                    #[derive(Display)]
                    enum Unit {
                        #[display("{I32:x?}")]
                        LowerDebug,
                        #[display("{I32:X?}")]
                        UpperDebug,
                        #[display("{:^}", I32)]
                        Align,
                        #[display("{:+}", I32)]
                        Sign,
                        #[display("{:#b}", I32)]
                        Alternate,
                        #[display("{:0}", I32)]
                        ZeroPadded,
                        #[display("{:07}", I32)]
                        Width,
                        #[display("{:.1}", F64)]
                        Precision,
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(format!("{:03}", Unit::LowerDebug), "b");
                        assert_eq!(format!("{:03}", Unit::UpperDebug), "B");
                        assert_eq!(format!("{:03}", Unit::Align), "11");
                        assert_eq!(format!("{:04}", Unit::Sign), "+11");
                        assert_eq!(format!("{:07}", Unit::Alternate), "0b1011");
                        assert_eq!(format!("{:07}", Unit::ZeroPadded), "11");
                        assert_eq!(format!("{:03}", Unit::Width), "0000011");
                        assert_eq!(format!("{:.3}", Unit::Precision), "3.1");
                    }
                }
            }
        }
    }

    mod single_field_variant {
        use super::*;

        #[derive(Display)]
        enum Enum {
            Unnamed(i32),
            Named {
                field: i32,
            },
            #[display("unnamed")]
            StrUnnamed(i32),
            #[display("named")]
            StrNamed {
                field: i32,
            },
            #[display("{_0} {}", _0)]
            InterpolatedUnnamed(i32),
            #[display("{field} {}", field)]
            InterpolatedNamed {
                field: i32,
            },
        }

        #[test]
        fn assert() {
            assert_eq!(Enum::Unnamed(1).to_string(), "1");
            assert_eq!(Enum::Named { field: 1 }.to_string(), "1");
            assert_eq!(Enum::StrUnnamed(1).to_string(), "unnamed");
            assert_eq!(Enum::StrNamed { field: 1 }.to_string(), "named");
            assert_eq!(Enum::InterpolatedUnnamed(1).to_string(), "1 1");
            assert_eq!(Enum::InterpolatedNamed { field: 1 }.to_string(), "1 1");
        }

        mod transparency {
            use super::*;

            mod direct {
                use super::*;

                #[derive(Display)]
                enum Display {
                    A(i32),
                    B { field: u8 },
                }

                #[derive(Binary)]
                enum Binary {
                    A(i32),
                    B { field: u8 },
                }

                #[derive(Octal)]
                enum Octal {
                    A(i32),
                    B { field: u8 },
                }

                #[derive(LowerHex)]
                enum LowerHex {
                    A(i32),
                    B { field: u8 },
                }

                #[derive(UpperHex)]
                enum UpperHex {
                    A(i32),
                    B { field: u8 },
                }

                #[derive(LowerExp)]
                enum LowerExp {
                    A(f64),
                    B { field: f32 },
                }

                #[derive(UpperExp)]
                enum UpperExp {
                    A(f64),
                    B { field: f32 },
                }

                #[derive(Pointer)]
                enum Pointer<'a> {
                    A(&'a i32),
                    B { field: &'a u8 },
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", Display::A(7)), "007");
                    assert_eq!(format!("{:03}", Display::B { field: 8 }), "008");
                    assert_eq!(format!("{:07b}", Binary::A(7)), "0000111");
                    assert_eq!(format!("{:07b}", Binary::B { field: 8 }), "0001000");
                    assert_eq!(format!("{:03o}", Octal::A(9)), "011");
                    assert_eq!(format!("{:03o}", Octal::B { field: 10 }), "012");
                    assert_eq!(format!("{:03x}", LowerHex::A(42)), "02a");
                    assert_eq!(format!("{:03x}", LowerHex::B { field: 43 }), "02b");
                    assert_eq!(format!("{:03X}", UpperHex::A(42)), "02A");
                    assert_eq!(format!("{:03X}", UpperHex::B { field: 43 }), "02B");
                    assert_eq!(format!("{:07e}", LowerExp::A(42.0)), "004.2e1");
                    assert_eq!(
                        format!("{:07e}", LowerExp::B { field: 43.0 }),
                        "004.3e1",
                    );
                    assert_eq!(format!("{:07E}", UpperExp::A(42.0)), "004.2E1");
                    assert_eq!(
                        format!("{:07E}", UpperExp::B { field: 43.0 }),
                        "004.3E1",
                    );
                    let (a, b) = (7, 42);
                    assert_eq!(
                        format!("{:018p}", Pointer::A(&a)),
                        format!("{:018p}", &a),
                    );
                    assert_eq!(
                        format!("{:018p}", Pointer::B { field: &b }),
                        format!("{:018p}", &b),
                    );
                }
            }

            mod interpolated {
                use super::*;

                #[derive(Display)]
                enum Display {
                    #[display("{_0}")]
                    A(i32),
                    #[display("{}", field)]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum Debug {
                    #[display("{0:?}", _0)]
                    A(i32),
                    #[display("{named:?}", named = field)]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum Binary {
                    #[display("{_0:b}")]
                    A(i32),
                    #[display("{:b}", field)]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum Octal {
                    #[display("{_0:o}")]
                    A(i32),
                    #[display("{:o}", field)]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum LowerHex {
                    #[display("{_0:x}")]
                    A(i32),
                    #[display("{:x}", field)]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum UpperHex {
                    #[display("{_0:X}")]
                    A(i32),
                    #[display("{:X}", field)]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum LowerExp {
                    #[display("{:e}", _0)]
                    A(f64),
                    #[display("{field:e}")]
                    B { field: f32 },
                }

                #[derive(Display)]
                enum UpperExp {
                    #[display("{:E}", _0)]
                    A(f64),
                    #[display("{field:E}")]
                    B { field: f32 },
                }

                #[derive(Display)]
                enum Pointer<'a> {
                    #[display("{:p}", *_0)]
                    A(&'a i32),
                    #[display("{field:p}")]
                    B { field: &'a u8 },
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", Display::A(7)), "007");
                    assert_eq!(format!("{:03}", Display::B { field: 8 }), "008");
                    assert_eq!(format!("{:03}", Debug::A(8)), "008");
                    assert_eq!(format!("{:03}", Debug::B { field: 9 }), "009");
                    assert_eq!(format!("{:07}", Binary::A(7)), "0000111");
                    assert_eq!(format!("{:07}", Binary::B { field: 8 }), "0001000");
                    assert_eq!(format!("{:03}", Octal::A(9)), "011");
                    assert_eq!(format!("{:03}", Octal::B { field: 10 }), "012");
                    assert_eq!(format!("{:03}", LowerHex::A(42)), "02a");
                    assert_eq!(format!("{:03}", LowerHex::B { field: 43 }), "02b");
                    assert_eq!(format!("{:03}", UpperHex::A(42)), "02A");
                    assert_eq!(format!("{:03}", UpperHex::B { field: 43 }), "02B");
                    assert_eq!(format!("{:07}", LowerExp::A(42.0)), "004.2e1");
                    assert_eq!(
                        format!("{:07}", LowerExp::B { field: 43.0 }),
                        "004.3e1",
                    );
                    assert_eq!(format!("{:07}", UpperExp::A(42.0)), "004.2E1");
                    assert_eq!(
                        format!("{:07}", UpperExp::B { field: 43.0 }),
                        "004.3E1",
                    );
                    let (a, b) = (7, 42);
                    assert_eq!(
                        format!("{:018}", Pointer::A(&a)),
                        format!("{:018p}", &a),
                    );
                    assert_eq!(
                        format!("{:018}", Pointer::B { field: &b }),
                        format!("{:018p}", &b),
                    );
                }
            }

            mod suppressed {
                use super::*;

                #[derive(Display)]
                enum Display {
                    #[display("{}", format_args!("{_0}"))]
                    A(i32),
                    #[display("{}", format_args!("{}", field))]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum Debug {
                    #[display("{}", format_args!("{_0:?}"))]
                    A(i32),
                    #[display("{}", format_args!("{:?}", field))]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum Binary {
                    #[display("{}", format_args!("{_0:b}"))]
                    A(i32),
                    #[display("{}", format_args!("{:b}", field))]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum Octal {
                    #[display("{}", format_args!("{_0:o}"))]
                    A(i32),
                    #[display("{}", format_args!("{:o}", field))]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum LowerHex {
                    #[display("{}", format_args!("{_0:x}"))]
                    A(i32),
                    #[display("{}", format_args!("{:x}", field))]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum UpperHex {
                    #[display("{}", format_args!("{_0:X}"))]
                    A(i32),
                    #[display("{}", format_args!("{:X}", field))]
                    B { field: u8 },
                }

                #[derive(Display)]
                enum LowerExp {
                    #[display("{}", format_args!("{:e}", _0))]
                    A(f64),
                    #[display("{}", format_args!("{field:e}"))]
                    B { field: f32 },
                }

                #[derive(Display)]
                enum UpperExp {
                    #[display("{}", format_args!("{:E}", _0))]
                    A(f64),
                    #[display("{}", format_args!("{field:E}"))]
                    B { field: f32 },
                }

                #[derive(Display)]
                enum Pointer<'a> {
                    #[display("{}", format_args!("{:p}", *_0))]
                    A(&'a i32),
                    #[display("{}", format_args!("{field:p}", field = *field))]
                    B { field: &'a u8 },
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", Display::A(7)), "7");
                    assert_eq!(format!("{:03}", Display::B { field: 8 }), "8");
                    assert_eq!(format!("{:03}", Debug::A(8)), "8");
                    assert_eq!(format!("{:03}", Debug::B { field: 9 }), "9");
                    assert_eq!(format!("{:07}", Binary::A(7)), "111");
                    assert_eq!(format!("{:07}", Binary::B { field: 8 }), "1000");
                    assert_eq!(format!("{:03}", Octal::A(9)), "11");
                    assert_eq!(format!("{:03}", Octal::B { field: 10 }), "12");
                    assert_eq!(format!("{:03}", LowerHex::A(42)), "2a");
                    assert_eq!(format!("{:03}", LowerHex::B { field: 43 }), "2b");
                    assert_eq!(format!("{:03}", UpperHex::A(42)), "2A");
                    assert_eq!(format!("{:03}", UpperHex::B { field: 43 }), "2B");
                    assert_eq!(format!("{:07}", LowerExp::A(42.0)), "4.2e1");
                    assert_eq!(format!("{:07}", LowerExp::B { field: 43.0 }), "4.3e1");
                    assert_eq!(format!("{:07}", UpperExp::A(42.0)), "4.2E1");
                    assert_eq!(format!("{:07}", UpperExp::B { field: 43.0 }), "4.3E1");
                    let (a, b) = (7, 42);
                    assert_eq!(format!("{:018}", Pointer::A(&a)), format!("{:0p}", &a));
                    assert_eq!(
                        format!("{:018}", Pointer::B { field: &b }),
                        format!("{:p}", &b),
                    );
                }
            }
        }
    }

    mod multi_field_variant {
        use super::*;

        #[derive(Display)]
        enum Enum {
            #[display("unnamed")]
            StrUnnamed(i32, i32),
            #[display("named")]
            StrNamed { field1: i32, field2: i32 },
            #[display(
                "{_0} {ident} {_1} {} {}",
                _1, _0 + _1, ident = 123, _1 = _0,
            )]
            InterpolatedUnnamed(i32, i32),
            #[display(
                "{field1} {ident} {field2} {} {}",
                field2, field1 + field2, ident = 123, field2 = field1,
            )]
            InterpolatedNamed { field1: i32, field2: i32 },
        }

        #[test]
        fn assert() {
            assert_eq!(Enum::StrUnnamed(1, 2).to_string(), "unnamed");
            assert_eq!(
                Enum::StrNamed {
                    field1: 1,
                    field2: 2,
                }
                .to_string(),
                "named",
            );
            assert_eq!(Enum::InterpolatedUnnamed(1, 2).to_string(), "1 123 1 2 3");
            assert_eq!(
                Enum::InterpolatedNamed {
                    field1: 1,
                    field2: 2,
                }
                .to_string(),
                "1 123 1 2 3",
            );
        }

        mod transparency {
            use super::*;

            mod interpolated {
                use super::*;

                #[derive(Display)]
                enum Display {
                    #[display("{_0}")]
                    A(i32, i64),
                    #[display("{}", b)]
                    B { a: u8, b: i32 },
                }

                #[derive(Display)]
                enum Debug {
                    #[display("{0:?}", _1)]
                    A(i32, i64),
                    #[display("{named:?}", named = a)]
                    B { a: u8, b: i32 },
                }

                #[derive(Display)]
                enum Binary {
                    #[display("{_0:b}")]
                    A(i32, i64),
                    #[display("{:b}", b)]
                    B { a: u8, b: i32 },
                }

                #[derive(Display)]
                enum Octal {
                    #[display("{_0:o}")]
                    A(i32, i64),
                    #[display("{:o}", b)]
                    B { a: u8, b: i32 },
                }

                #[derive(Display)]
                enum LowerHex {
                    #[display("{_0:x}")]
                    A(i32, i64),
                    #[display("{:x}", b)]
                    B { a: u8, b: i32 },
                }

                #[derive(Display)]
                enum UpperHex {
                    #[display("{_0:X}")]
                    A(i32, i64),
                    #[display("{:X}", b)]
                    B { a: u8, b: i32 },
                }

                #[derive(Display)]
                enum LowerExp {
                    #[display("{:e}", _1)]
                    A(f64, f32),
                    #[display("{a:e}")]
                    B { a: f64, b: f32 },
                }

                #[derive(Display)]
                enum UpperExp {
                    #[display("{:E}", _1)]
                    A(f64, f32),
                    #[display("{a:E}")]
                    B { a: f64, b: f32 },
                }

                #[derive(Display)]
                enum Pointer<'a> {
                    #[display("{:p}", *_1)]
                    A(&'a f64, &'a f32),
                    #[display("{a:p}")]
                    B { a: &'a f64, b: &'a f32 },
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03}", Display::A(7, 8)), "007");
                    assert_eq!(format!("{:03}", Display::B { a: 7, b: 8 }), "008");
                    assert_eq!(format!("{:03}", Debug::A(7, 8)), "008");
                    assert_eq!(format!("{:03}", Debug::B { a: 7, b: 8 }), "007");
                    assert_eq!(format!("{:07}", Binary::A(7, 8)), "0000111");
                    assert_eq!(format!("{:07}", Binary::B { a: 7, b: 8 }), "0001000");
                    assert_eq!(format!("{:03}", Octal::A(9, 10)), "011");
                    assert_eq!(format!("{:03}", Octal::B { a: 9, b: 10 }), "012");
                    assert_eq!(format!("{:03}", LowerHex::A(42, 41)), "02a");
                    assert_eq!(format!("{:03}", LowerHex::B { a: 42, b: 43 }), "02b");
                    assert_eq!(format!("{:03}", UpperHex::A(42, 41)), "02A");
                    assert_eq!(format!("{:03}", UpperHex::B { a: 42, b: 43 }), "02B");
                    assert_eq!(format!("{:07}", LowerExp::A(41.0, 42.0)), "004.2e1");
                    assert_eq!(
                        format!("{:07}", LowerExp::B { a: 43.0, b: 52.0 }),
                        "004.3e1",
                    );
                    assert_eq!(format!("{:07}", UpperExp::A(41.0, 42.0)), "004.2E1");
                    assert_eq!(
                        format!("{:07}", UpperExp::B { a: 43.0, b: 52.0 }),
                        "004.3E1",
                    );
                    let (a, b) = (8.3, 42.1);
                    assert_eq!(
                        format!("{:018}", Pointer::A(&7.0, &a)),
                        format!("{:018p}", &a),
                    );
                    assert_eq!(
                        format!("{:018}", Pointer::B { a: &b, b: &43.3 }),
                        format!("{:018p}", &b),
                    );
                }
            }
        }

        mod shared_format {
            use super::*;

            mod single {
                use super::*;

                #[derive(Display)]
                #[display("Variant: {_variant}")]
                enum Enum {
                    #[display("A {_0}")]
                    A(i32),
                    #[display("B {}", field)]
                    B {
                        field: i32,
                    },
                    C,
                }

                #[test]
                fn assert() {
                    assert_eq!(Enum::A(1).to_string(), "Variant: A 1");
                    assert_eq!(Enum::B { field: 2 }.to_string(), "Variant: B 2");
                    assert_eq!(Enum::C.to_string(), "Variant: C");
                }
            }

            mod transparent {
                use super::*;

                #[derive(Display)]
                #[display("{_variant}")]
                enum Enum {
                    #[display("A {_0}")]
                    A(i32),
                    #[display("B {}", field)]
                    B {
                        field: i32,
                    },
                    C,
                    #[display("{_0:b}")]
                    TransparentBinary(i32),
                }

                #[test]
                fn assert() {
                    assert_eq!(Enum::A(1).to_string(), "A 1");
                    assert_eq!(Enum::B { field: 2 }.to_string(), "B 2");
                    assert_eq!(Enum::C.to_string(), "C");
                    assert_eq!(
                        format!("{:08}", Enum::TransparentBinary(4)),
                        "00000100",
                    );
                }
            }

            mod multiple {
                use super::*;

                #[derive(Display)]
                #[display("{_variant} Variant: {_variant} {}", _variant)]
                enum Enum {
                    #[display("A {_0}")]
                    A(i32),
                    #[display("B {}", field)]
                    B {
                        field: i32,
                    },
                    C,
                }

                #[test]
                fn assert() {
                    assert_eq!(Enum::A(1).to_string(), "A 1 Variant: A 1 A 1");
                    assert_eq!(
                        Enum::B { field: 2 }.to_string(),
                        "B 2 Variant: B 2 B 2",
                    );
                    assert_eq!(Enum::C.to_string(), "C Variant: C C");
                }
            }

            mod none {
                use super::*;

                /// Make sure that variant-specific bounds are not added if `_variant` is not used.
                struct NoDisplay;

                #[derive(Display)]
                #[display("Variant")]
                enum Enum<T> {
                    #[display("A {_0}")]
                    A(i32),
                    #[display("B {}", field)]
                    B {
                        field: i32,
                    },
                    C,
                    D(T),
                }

                #[test]
                fn assert() {
                    assert_eq!(Enum::<NoDisplay>::A(1).to_string(), "Variant");
                    assert_eq!(
                        Enum::<NoDisplay>::B { field: 2 }.to_string(),
                        "Variant",
                    );
                    assert_eq!(Enum::<NoDisplay>::C.to_string(), "Variant");
                    assert_eq!(Enum::<NoDisplay>::D(NoDisplay).to_string(), "Variant");
                }
            }

            mod use_field {
                use super::*;

                #[derive(Display)]
                #[display("Variant {_0}")]
                enum Enum<T> {
                    A(i32),
                    B(&'static str),
                    C(T),
                }

                #[test]
                fn assert() {
                    assert_eq!(Enum::<u8>::A(1).to_string(), "Variant 1");
                    assert_eq!(Enum::<u8>::B("abc").to_string(), "Variant abc");
                    assert_eq!(Enum::<u8>::C(9).to_string(), "Variant 9");
                }
            }

            mod use_field_and_variant {
                use super::*;

                #[derive(Display)]
                #[display("Variant {_variant} {}", _0)]
                enum Enum<T> {
                    #[display("A")]
                    A(i32),
                    #[display("B")]
                    B(&'static str),
                    #[display("C")]
                    C(T),
                }

                #[test]
                fn assert() {
                    assert_eq!(Enum::<u8>::A(1).to_string(), "Variant A 1");
                    assert_eq!(Enum::<u8>::B("abc").to_string(), "Variant B abc");
                    assert_eq!(Enum::<u8>::C(9).to_string(), "Variant C 9");
                }
            }

            mod pointer {
                use super::*;

                #[derive(Display)]
                #[display("Pointer {_0:p} {_variant} {_0:p}")]
                enum Pointer<'a> {
                    #[display("A")]
                    A(&'a f64),
                    #[display("B")]
                    B(&'a f32),
                }
                #[test]
                fn assert() {
                    let (a, b) = (8.3, 42.1);
                    assert_eq!(
                        Pointer::A(&a).to_string(),
                        format!("Pointer {0:p} A {0:p}", &a),
                    );
                    assert_eq!(
                        Pointer::B(&b).to_string(),
                        format!("Pointer {0:p} B {0:p}", &b),
                    );
                }
            }
        }
    }
}

mod generic {
    use super::*;

    trait Bound {}

    impl Bound for () {}

    fn display_bound<T: Bound>(_: &T) -> &'static str {
        "()"
    }

    #[derive(Display)]
    #[display("Generic {}", field)]
    struct NamedGenericStruct<T> {
        field: T,
    }
    #[test]
    fn named_generic_struct() {
        assert_eq!(NamedGenericStruct { field: 1 }.to_string(), "Generic 1");
    }

    #[derive(Display)]
    #[display("Generic {field}")]
    struct InterpolatedNamedGenericStruct<T> {
        field: T,
    }
    #[test]
    fn interpolated_named_generic_struct() {
        assert_eq!(
            InterpolatedNamedGenericStruct { field: 1 }.to_string(),
            "Generic 1",
        );
    }

    #[derive(Display)]
    #[display("Generic {field:<>width$.prec$} {field}")]
    struct InterpolatedNamedGenericStructWidthPrecision<T> {
        field: T,
        width: usize,
        prec: usize,
    }
    #[test]
    fn interpolated_named_generic_struct_width_precision() {
        assert_eq!(
            InterpolatedNamedGenericStructWidthPrecision {
                field: 1.2345,
                width: 9,
                prec: 2,
            }
            .to_string(),
            "Generic <<<<<1.23 1.2345",
        );
    }

    #[derive(Display)]
    struct AutoNamedGenericStruct<T> {
        field: T,
    }
    #[test]
    fn auto_named_generic_struct() {
        assert_eq!(AutoNamedGenericStruct { field: 1 }.to_string(), "1");
    }

    #[derive(Display)]
    #[display("{alias}", alias = field)]
    struct AliasedNamedGenericStruct<T> {
        field: T,
    }
    #[test]
    fn aliased_named_generic_struct() {
        assert_eq!(AliasedNamedGenericStruct { field: 1 }.to_string(), "1");
    }

    #[derive(Display)]
    #[display("{field1}", field1 = field2)]
    struct AliasedFieldNamedGenericStruct<T> {
        field1: T,
        field2: i32,
    }
    #[test]
    fn aliased_field_named_generic_struct() {
        assert_eq!(
            AliasedFieldNamedGenericStruct {
                field1: (),
                field2: 1,
            }
            .to_string(),
            "1",
        );
    }

    #[derive(Display)]
    #[display("Generic {}", _0)]
    struct UnnamedGenericStruct<T>(T);
    #[test]
    fn unnamed_generic_struct() {
        assert_eq!(UnnamedGenericStruct(2).to_string(), "Generic 2");
    }

    #[derive(Display)]
    #[display("Generic {_0}")]
    struct InterpolatedUnnamedGenericStruct<T>(T);
    #[test]
    fn interpolated_unnamed_generic_struct() {
        assert_eq!(InterpolatedUnnamedGenericStruct(2).to_string(), "Generic 2");
    }

    #[derive(Display)]
    struct AutoUnnamedGenericStruct<T>(T);
    #[test]
    fn auto_unnamed_generic_struct() {
        assert_eq!(AutoUnnamedGenericStruct(2).to_string(), "2");
    }

    #[derive(Display)]
    #[display("{alias}", alias = _0)]
    struct AliasedUnnamedGenericStruct<T>(T);
    #[test]
    fn aliased_unnamed_generic_struct() {
        assert_eq!(AliasedUnnamedGenericStruct(2).to_string(), "2");
    }

    #[derive(Display)]
    #[display("{_0}", _0 = _1)]
    struct AliasedFieldUnnamedGenericStruct<T>(T, i32);
    #[test]
    fn aliased_field_unnamed_generic_struct() {
        assert_eq!(AliasedFieldUnnamedGenericStruct((), 2).to_string(), "2");
    }

    #[derive(Display)]
    enum GenericEnum<A, B> {
        #[display("Gen::A {}", field)]
        A { field: A },
        #[display("Gen::B {}", _0)]
        B(B),
    }
    #[test]
    fn generic_enum() {
        assert_eq!(GenericEnum::A::<_, u8> { field: 1 }.to_string(), "Gen::A 1");
        assert_eq!(GenericEnum::B::<u8, _>(2).to_string(), "Gen::B 2");
    }

    #[derive(Display)]
    enum InterpolatedGenericEnum<A, B> {
        #[display("Gen::A {field}")]
        A { field: A },
        #[display("Gen::B {_0}")]
        B(B),
    }
    #[test]
    fn interpolated_generic_enum() {
        assert_eq!(
            InterpolatedGenericEnum::A::<_, u8> { field: 1 }.to_string(),
            "Gen::A 1",
        );
        assert_eq!(
            InterpolatedGenericEnum::B::<u8, _>(2).to_string(),
            "Gen::B 2",
        );
    }

    #[derive(Display)]
    enum AutoGenericEnum<A, B> {
        A { field: A },
        B(B),
    }
    #[test]
    fn auto_generic_enum() {
        assert_eq!(AutoGenericEnum::A::<_, u8> { field: 1 }.to_string(), "1");
        assert_eq!(AutoGenericEnum::B::<u8, _>(2).to_string(), "2");
    }

    #[derive(Display)]
    #[display("{} {} <-> {0:o} {1:#x} <-> {0:?} {1:X?}", a, b)]
    struct MultiTraitNamedGenericStruct<A, B> {
        a: A,
        b: B,
    }
    #[test]
    fn multi_trait_named_generic_struct() {
        let s = MultiTraitNamedGenericStruct { a: 8u8, b: 255 };
        assert_eq!(s.to_string(), "8 255 <-> 10 0xff <-> 8 FF");
    }

    #[derive(Display)]
    #[display("{} {b} <-> {0:o} {1:#x} <-> {0:?} {1:X?}", a, b)]
    struct InterpolatedMultiTraitNamedGenericStruct<A, B> {
        a: A,
        b: B,
    }
    #[test]
    fn interpolated_multi_trait_named_generic_struct() {
        let s = InterpolatedMultiTraitNamedGenericStruct { a: 8u8, b: 255 };
        assert_eq!(s.to_string(), "8 255 <-> 10 0xff <-> 8 FF");
    }

    #[derive(Display)]
    #[display("{} {} {{}} {0:o} {1:#x} - {0:>4?} {1:^4X?}", _0, _1)]
    struct MultiTraitUnnamedGenericStruct<A, B>(A, B);
    #[test]
    fn multi_trait_unnamed_generic_struct() {
        let s = MultiTraitUnnamedGenericStruct(8u8, 255);
        assert_eq!(s.to_string(), "8 255 {} 10 0xff -    8  FF ");
    }

    #[derive(Display)]
    #[display("{} {_1} {{}} {0:o} {1:#x} - {0:>4?} {1:^4X?}", _0, _1)]
    struct InterpolatedMultiTraitUnnamedGenericStruct<A, B>(A, B);
    #[test]
    fn interpolated_multi_trait_unnamed_generic_struct() {
        let s = InterpolatedMultiTraitUnnamedGenericStruct(8u8, 255);
        assert_eq!(s.to_string(), "8 255 {} 10 0xff -    8  FF ");
    }

    #[derive(Display)]
    #[display("{}", 3 * 4)]
    struct UnusedGenericStruct<T>(T);
    #[test]
    fn unused_generic_struct() {
        let s = UnusedGenericStruct(());
        assert_eq!(s.to_string(), "12");
    }

    mod associated_type_field_enumerator {
        use super::*;

        trait Trait {
            type Type;
        }

        struct Struct;

        impl Trait for Struct {
            type Type = i32;
        }

        #[test]
        fn auto_generic_named_struct_associated() {
            #[derive(Display)]
            struct AutoGenericNamedStructAssociated<T: Trait> {
                field: <T as Trait>::Type,
            }

            let s = AutoGenericNamedStructAssociated::<Struct> { field: 10 };
            assert_eq!(s.to_string(), "10");
        }

        #[test]
        fn auto_generic_unnamed_struct_associated() {
            #[derive(Display)]
            struct AutoGenericUnnamedStructAssociated<T: Trait>(<T as Trait>::Type);

            let s = AutoGenericUnnamedStructAssociated::<Struct>(10);
            assert_eq!(s.to_string(), "10");
        }

        #[test]
        fn auto_generic_enum_associated() {
            #[derive(Display)]
            enum AutoGenericEnumAssociated<T: Trait> {
                Enumerator(<T as Trait>::Type),
            }

            let e = AutoGenericEnumAssociated::<Struct>::Enumerator(10);
            assert_eq!(e.to_string(), "10");
        }
    }

    mod complex_type_field_enumerator {
        use super::*;

        #[derive(Display)]
        struct Struct<T>(T);

        #[test]
        fn auto_generic_named_struct_complex() {
            #[derive(Display)]
            struct AutoGenericNamedStructComplex<T> {
                field: Struct<T>,
            }

            let s = AutoGenericNamedStructComplex { field: Struct(10) };
            assert_eq!(s.to_string(), "10");
        }

        #[test]
        fn auto_generic_unnamed_struct_complex() {
            #[derive(Display)]
            struct AutoGenericUnnamedStructComplex<T>(Struct<T>);

            let s = AutoGenericUnnamedStructComplex(Struct(10));
            assert_eq!(s.to_string(), "10");
        }

        #[test]
        fn auto_generic_enum_complex() {
            #[derive(Display)]
            enum AutoGenericEnumComplex<T> {
                Enumerator(Struct<T>),
            }

            let e = AutoGenericEnumComplex::Enumerator(Struct(10));
            assert_eq!(e.to_string(), "10")
        }
    }

    mod reference {
        use super::*;

        #[test]
        fn auto_generic_reference() {
            #[derive(Display)]
            struct AutoGenericReference<'a, T>(&'a T);

            let s = AutoGenericReference(&10);
            assert_eq!(s.to_string(), "10");
        }

        #[test]
        fn auto_generic_static_reference() {
            #[derive(Display)]
            struct AutoGenericStaticReference<T: 'static>(&'static T);

            let s = AutoGenericStaticReference(&10);
            assert_eq!(s.to_string(), "10");
        }
    }

    mod indirect {
        use super::*;

        #[derive(Display)]
        struct Struct<T>(T);

        #[test]
        fn auto_generic_indirect() {
            #[derive(Display)]
            struct AutoGenericIndirect<T: 'static>(Struct<&'static T>);

            const V: i32 = 10;
            let s = AutoGenericIndirect(Struct(&V));
            assert_eq!(s.to_string(), "10");
        }
    }

    mod bound {
        use super::*;

        #[test]
        fn simple() {
            #[derive(Display)]
            #[display("{} {}", _0, _1)]
            struct Struct<T1, T2>(T1, T2);

            let s = Struct(10, 20);
            assert_eq!(s.to_string(), "10 20");
        }

        #[test]
        fn debug() {
            #[derive(Debug)]
            struct OnlyDebug;

            #[derive(Display)]
            #[display("{:?}", _0)]
            struct Struct<T>(T);

            let s = Struct(OnlyDebug);
            assert_eq!(s.to_string(), "OnlyDebug");
        }

        #[test]
        fn underscored_simple() {
            #[derive(Display)]
            #[display("{_0} {_1}")]
            struct Struct<T1, T2>(T1, T2);

            let s = Struct(10, 20);
            assert_eq!(s.to_string(), "10 20");
        }

        #[test]
        fn underscored_debug() {
            #[derive(Debug)]
            struct OnlyDebug;

            #[derive(Display)]
            #[display("{_0:?}")]
            struct Struct<T>(T);

            let s = Struct(OnlyDebug);
            assert_eq!(s.to_string(), "OnlyDebug");
        }

        #[test]
        fn redundant() {
            #[derive(Display)]
            #[display(bound(T1: ::core::fmt::Display, T2: ::core::fmt::Debug))]
            #[display("{} {:?}", _0, _1)]
            struct Struct<T1, T2>(T1, T2);

            let s = Struct(10, 20);
            assert_eq!(s.to_string(), "10 20");
        }

        #[test]
        fn underscored_redundant() {
            #[derive(Display)]
            #[display(bound(T1: ::core::fmt::Display, T2: ::core::fmt::Debug))]
            #[display("{_0} {_1:?}")]
            struct Struct<T1, T2>(T1, T2);

            let s = Struct(10, 20);
            assert_eq!(s.to_string(), "10 20");
        }

        #[test]
        fn complex() {
            #[derive(Debug)]
            struct DebugOnly;

            trait Trait1 {
                fn function1(&self) -> &'static str;
            }

            trait Trait2 {
                fn function2(&self) -> &'static str;
            }

            impl Trait1 for i32 {
                fn function1(&self) -> &'static str {
                    "WHAT"
                }
            }

            impl Trait2 for i32 {
                fn function2(&self) -> &'static str {
                    "EVER"
                }
            }

            impl Trait1 for DebugOnly {
                fn function1(&self) -> &'static str {
                    "MAN"
                }
            }

            #[derive(Display)]
            #[display("{} {} {} {} {:?}", _0.function1(), _0, _0.function2(), _1.function1(), _1)]
            #[display(bound(T1: Trait1 + Trait2, T2: Trait1))]
            struct Struct<T1, T2>(T1, T2);

            let s = Struct(10, DebugOnly);
            assert_eq!(s.to_string(), "WHAT 10 EVER MAN DebugOnly");
        }

        #[test]
        fn underscored_complex() {
            #[derive(Debug)]
            struct DebugOnly;

            trait Trait1 {
                fn function1(&self) -> &'static str;
            }

            trait Trait2 {
                fn function2(&self) -> &'static str;
            }

            impl Trait1 for i32 {
                fn function1(&self) -> &'static str {
                    "WHAT"
                }
            }

            impl Trait2 for i32 {
                fn function2(&self) -> &'static str {
                    "EVER"
                }
            }

            impl Trait1 for DebugOnly {
                fn function1(&self) -> &'static str {
                    "MAN"
                }
            }

            #[derive(Display)]
            #[display(bound(T1: Trait1 + Trait2, T2: Trait1))]
            #[display("{} {_0} {} {} {_1:?}", _0.function1(), _0.function2(), _1.function1())]
            struct Struct<T1, T2>(T1, T2);

            let s = Struct(10, DebugOnly);
            assert_eq!(s.to_string(), "WHAT 10 EVER MAN DebugOnly");
        }

        #[test]
        fn explicit_only() {
            trait Trait {
                fn function(&self) -> &'static str;
            }

            struct NoDisplay;

            impl Trait for NoDisplay {
                fn function(&self) -> &'static str {
                    "no display"
                }
            }

            #[derive(Display)]
            #[display("{}", _0.function())]
            #[display(bound(T: Trait))]
            struct Struct<T>(T);

            let s = Struct(NoDisplay);
            assert_eq!(s.to_string(), "no display");
        }
    }

    mod transparency {
        use super::*;

        mod direct {
            use super::*;

            #[derive(
                Display, Binary, Octal, LowerHex, UpperHex, LowerExp, UpperExp, Pointer
            )]
            struct Tuple<T>(T);

            #[derive(
                Display, Binary, Octal, LowerHex, UpperHex, LowerExp, UpperExp, Pointer
            )]
            struct Struct<T> {
                field: T,
            }

            #[derive(
                Display, Binary, Octal, LowerHex, UpperHex, LowerExp, UpperExp, Pointer
            )]
            enum Enum<A, B> {
                A(A),
                B { field: B },
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:03}", Tuple(7)), "007");
                assert_eq!(format!("{:03}", Struct { field: 7 }), "007");
                assert_eq!(format!("{:03}", Enum::<_, i8>::A(7)), "007");
                assert_eq!(format!("{:03}", Enum::<i8, _>::B { field: 8 }), "008");
                assert_eq!(format!("{:07b}", Tuple(7)), "0000111");
                assert_eq!(format!("{:07b}", Struct { field: 7 }), "0000111");
                assert_eq!(format!("{:07b}", Enum::<_, i8>::A(7)), "0000111");
                assert_eq!(format!("{:07b}", Enum::<i8, _>::B { field: 8 }), "0001000");
                assert_eq!(format!("{:03o}", Tuple(9)), "011");
                assert_eq!(format!("{:03o}", Struct { field: 9 }), "011");
                assert_eq!(format!("{:03o}", Enum::<_, i8>::A(9)), "011");
                assert_eq!(format!("{:03o}", Enum::<i8, _>::B { field: 10 }), "012");
                assert_eq!(format!("{:03x}", Tuple(42)), "02a");
                assert_eq!(format!("{:03x}", Struct { field: 42 }), "02a");
                assert_eq!(format!("{:03x}", Enum::<_, i8>::A(42)), "02a");
                assert_eq!(format!("{:03x}", Enum::<i8, _>::B { field: 43 }), "02b");
                assert_eq!(format!("{:03X}", Tuple(42)), "02A");
                assert_eq!(format!("{:03X}", Struct { field: 42 }), "02A");
                assert_eq!(format!("{:03X}", Enum::<_, i8>::A(42)), "02A");
                assert_eq!(format!("{:03X}", Enum::<i8, _>::B { field: 43 }), "02B");
                assert_eq!(format!("{:07e}", Tuple(42.0)), "004.2e1");
                assert_eq!(format!("{:07e}", Struct { field: 42.0 }), "004.2e1");
                assert_eq!(format!("{:07e}", Enum::<_, i8>::A(42.0)), "004.2e1");
                assert_eq!(
                    format!("{:07e}", Enum::<i8, _>::B { field: 43.0 }),
                    "004.3e1",
                );
                assert_eq!(format!("{:07E}", Tuple(42.0)), "004.2E1");
                assert_eq!(format!("{:07E}", Struct { field: 42.0 }), "004.2E1");
                assert_eq!(format!("{:07E}", Enum::<_, i8>::A(42.0)), "004.2E1");
                assert_eq!(
                    format!("{:07E}", Enum::<i8, _>::B { field: 43.0 }),
                    "004.3E1",
                );
                let (a, b) = (42, 7);
                assert_eq!(format!("{:018p}", Tuple(&a)), format!("{:018p}", &a));
                assert_eq!(
                    format!("{:018p}", Struct { field: &a }),
                    format!("{:018p}", &a),
                );
                assert_eq!(
                    format!("{:018p}", Enum::<_, &i8>::A(&b)),
                    format!("{:018p}", &b),
                );
                assert_eq!(
                    format!("{:018p}", Enum::<&i8, _>::B { field: &a }),
                    format!("{:018p}", &a),
                );
            }
        }

        mod interpolated {
            use super::*;

            #[derive(Display)]
            #[display("{_0}")]
            struct TupleDisplay<T>(T);

            #[derive(Display)]
            #[display("{0:?}", _0)]
            struct TupleDebug<T>(T);

            #[derive(Display)]
            #[display("{_0:b}")]
            struct TupleBinary<T, Y>(T, Y);

            #[derive(Display)]
            #[display("{_1:o}")]
            struct TupleOctal<Y, T>(Y, T);

            #[derive(Display)]
            #[display("{0:x}", _0)]
            struct TupleLowerHex<T>(T);

            #[derive(Display)]
            #[display("{_0:X}")]
            struct TupleUpperHex<T>(T);

            #[derive(Display)]
            #[display("{:e}", _0)]
            struct TupleLowerExp<T>(T);

            #[derive(Display)]
            #[display("{:E}", _0)]
            struct TupleUpperExp<T>(T);

            #[derive(Display)]
            #[display("{_0:p}")]
            struct TuplePointer<T>(T);

            #[derive(Display)]
            #[display("{field}")]
            struct StructDisplay<T> {
                field: T,
            }

            #[derive(Display)]
            #[display("{a:b}")]
            struct StructBinary<T, Y> {
                a: T,
                b: Y,
            }

            #[derive(Display)]
            #[display("{named:o}", named = b)]
            struct StructOctal<Y, T> {
                a: Y,
                b: T,
            }

            #[derive(Display)]
            #[display("{field:x}")]
            struct StructLowerHex<T> {
                field: T,
            }

            #[derive(Display)]
            #[display("{field:X}")]
            struct StructUpperHex<T> {
                field: T,
            }

            #[derive(Display)]
            #[display("{:e}", field)]
            struct StructLowerExp<T> {
                field: T,
            }

            #[derive(Display)]
            #[display("{:E}", field)]
            struct StructUpperExp<T> {
                field: T,
            }

            #[derive(Display)]
            #[display("{field:p}")]
            struct StructPointer<T> {
                field: T,
            }

            #[derive(Display)]
            enum EnumDisplay<A, B> {
                #[display("{_0}")]
                A(A),
                #[display("{}", field)]
                B { field: B },
            }

            #[derive(Display)]
            enum EnumBinary<A, B, C, D> {
                #[display("{_0:b}")]
                A(A, C),
                #[display("{:b}", b)]
                B { b: B, d: D },
            }

            #[derive(Display)]
            enum EnumOctal<A, B, C, D> {
                #[display("{_1:o}")]
                A(A, C),
                #[display("{:o}", d)]
                B { b: B, d: D },
            }

            #[derive(Display)]
            enum EnumLowerHex<A, B> {
                #[display("{_0:x}")]
                A(A),
                #[display("{:x}", field)]
                B { field: B },
            }

            #[derive(Display)]
            enum EnumUpperHex<A, B> {
                #[display("{_0:X}")]
                A(A),
                #[display("{:X}", field)]
                B { field: B },
            }

            #[derive(Display)]
            enum EnumLowerExp<A, B> {
                #[display("{:e}", _0)]
                A(A),
                #[display("{field:e}")]
                B { field: B },
            }

            #[derive(Display)]
            enum EnumUpperExp<A, B> {
                #[display("{:E}", _0)]
                A(A),
                #[display("{field:E}")]
                B { field: B },
            }

            #[derive(Display)]
            enum EnumPointer<A, B> {
                #[display("{_0:p}")]
                A(A),
                #[display("{field:p}")]
                B { field: B },
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:03}", TupleDisplay(7)), "007");
                assert_eq!(format!("{:03}", TupleDebug(8)), "008");
                assert_eq!(format!("{:03}", StructDisplay { field: 7 }), "007");
                assert_eq!(format!("{:03}", EnumDisplay::<_, i8>::A(7)), "007");
                assert_eq!(
                    format!("{:03}", EnumDisplay::<i8, _>::B { field: 8 }),
                    "008",
                );
                assert_eq!(format!("{:07}", TupleBinary(7, ())), "0000111");
                assert_eq!(format!("{:07}", StructBinary { a: 7, b: () }), "0000111");
                assert_eq!(
                    format!("{:07}", EnumBinary::<_, i8, _, ()>::A(7, ())),
                    "0000111",
                );
                assert_eq!(
                    format!("{:07}", EnumBinary::<i8, _, (), _>::B { b: 8, d: () }),
                    "0001000",
                );
                assert_eq!(format!("{:03}", TupleOctal((), 9)), "011");
                assert_eq!(format!("{:03}", StructOctal { a: (), b: 9 }), "011");
                assert_eq!(
                    format!("{:03}", EnumOctal::<_, (), _, i8>::A((), 9)),
                    "011",
                );
                assert_eq!(
                    format!("{:03}", EnumOctal::<(), _, i8, _>::B { b: (), d: 10 }),
                    "012",
                );
                assert_eq!(format!("{:03}", TupleLowerHex(42)), "02a");
                assert_eq!(format!("{:03}", StructLowerHex { field: 42 }), "02a");
                assert_eq!(format!("{:03}", EnumLowerHex::<_, i8>::A(42)), "02a");
                assert_eq!(
                    format!("{:03}", EnumLowerHex::<i8, _>::B { field: 43 }),
                    "02b",
                );
                assert_eq!(format!("{:03}", TupleUpperHex(42)), "02A");
                assert_eq!(format!("{:03}", StructUpperHex { field: 42 }), "02A");
                assert_eq!(format!("{:03}", EnumUpperHex::<_, i8>::A(42)), "02A");
                assert_eq!(
                    format!("{:03}", EnumUpperHex::<i8, _>::B { field: 43 }),
                    "02B",
                );
                assert_eq!(format!("{:07}", TupleLowerExp(42.0)), "004.2e1");
                assert_eq!(format!("{:07}", StructLowerExp { field: 42.0 }), "004.2e1");
                assert_eq!(format!("{:07}", EnumLowerExp::<_, i8>::A(42.0)), "004.2e1");
                assert_eq!(
                    format!("{:07}", EnumLowerExp::<i8, _>::B { field: 43.0 }),
                    "004.3e1",
                );
                assert_eq!(format!("{:07}", TupleUpperExp(42.0)), "004.2E1");
                assert_eq!(format!("{:07}", StructUpperExp { field: 42.0 }), "004.2E1");
                assert_eq!(format!("{:07}", EnumUpperExp::<_, i8>::A(42.0)), "004.2E1");
                assert_eq!(
                    format!("{:07}", EnumUpperExp::<i8, _>::B { field: 43.0 }),
                    "004.3E1",
                );
                let (a, b) = (42, 7);
                assert_eq!(format!("{:018}", TuplePointer(&a)), format!("{:018p}", &a));
                assert_eq!(
                    format!("{:018}", StructPointer { field: &a }),
                    format!("{:018p}", &a),
                );
                assert_eq!(
                    format!("{:018}", EnumPointer::<_, &i8>::A(&b)),
                    format!("{:018p}", &b),
                );
                assert_eq!(
                    format!("{:018}", EnumPointer::<&i8, _>::B { field: &a }),
                    format!("{:018p}", &a),
                );
            }
        }

        mod omitted {
            use super::*;

            mod on_modifiers {
                use super::*;

                #[derive(Display)]
                enum Enum<A, B, C, D> {
                    #[display("{_0:x?}")]
                    LowerDebug(A),
                    #[display("{_0:X?}")]
                    UpperDebug(B),
                    #[display("{:^}", _0)]
                    Align(C),
                    #[display("{:+}", _0)]
                    Sign(C),
                    #[display("{:#b}", _0)]
                    Alternate(C),
                    #[display("{:0}", _0)]
                    ZeroPadded(C),
                    #[display("{:07}", _0)]
                    Width(C),
                    #[display("{:.5}", _0)]
                    Precision(D),
                }

                #[test]
                fn assert() {
                    assert_eq!(
                        format!("{:03}", Enum::<_, u8, u8, f64>::LowerDebug(7)),
                        "7",
                    );
                    assert_eq!(
                        format!("{:03}", Enum::<u8, _, u8, f64>::UpperDebug(8)),
                        "8",
                    );
                    assert_eq!(format!("{:03}", Enum::<u8, u8, _, f64>::Align(5)), "5");
                    assert_eq!(format!("{:03}", Enum::<u8, u8, _, f64>::Sign(5)), "+5");
                    assert_eq!(
                        format!("{:07}", Enum::<u8, u8, _, f64>::Alternate(5)),
                        "0b101",
                    );
                    assert_eq!(
                        format!("{:07}", Enum::<u8, u8, _, f64>::ZeroPadded(-5)),
                        "-5",
                    );
                    assert_eq!(
                        format!("{:03}", Enum::<u8, u8, _, f64>::Width(5)),
                        "0000005",
                    );
                    assert_eq!(
                        format!("{:.3}", Enum::<u8, u8, u8, _>::Precision(1.23456789)),
                        "1.23457",
                    );
                }
            }
        }
    }
}

// See: https://github.com/JelteF/derive_more/issues/363
mod type_variables {
    mod our_alloc {
        #[cfg(not(feature = "std"))]
        pub use alloc::{boxed::Box, format, vec::Vec};
        #[cfg(feature = "std")]
        pub use std::{boxed::Box, format, vec::Vec};
    }

    use our_alloc::{format, Box};

    // We want `Vec` in scope to test that code generation works if it is there.
    #[allow(unused_imports)]
    use our_alloc::Vec;

    use derive_more::Display;

    #[derive(Display, Debug)]
    #[display("{inner:?}")]
    #[display(bounds(T: Display))]
    struct OptionalBox<T> {
        inner: Option<Box<T>>,
    }

    #[derive(Display, Debug)]
    #[display("{next}")]
    struct ItemStruct {
        next: OptionalBox<ItemStruct>,
    }

    #[derive(Display)]
    #[derive(Debug)]
    struct ItemTuple(OptionalBox<ItemTuple>);

    #[derive(Display)]
    #[derive(Debug)]
    #[display("Item({_0})")]
    struct ItemTupleContainerFmt(OptionalBox<ItemTupleContainerFmt>);

    #[derive(Display, Debug)]
    #[display("{next}")]
    enum ItemEnumOuterFormat {
        Variant1 {
            next: OptionalBox<ItemEnumOuterFormat>,
        },
        Variant2 {
            next: OptionalBox<i32>,
        },
    }

    #[derive(Display, Debug)]
    enum ItemEnumInnerFormat {
        #[display("{next} {inner}")]
        Node {
            next: OptionalBox<ItemEnumInnerFormat>,
            inner: i32,
        },
        #[display("{inner}")]
        Leaf { inner: i32 },
    }

    #[derive(Display)]
    #[derive(Debug)]
    #[display("{next:?}, {real:?}")]
    struct VecMeansDifferent<Vec> {
        next: our_alloc::Vec<i32>,
        real: Vec,
    }

    #[derive(Display)]
    #[derive(Debug)]
    #[display("{t:?}")]
    struct Array<T> {
        t: [T; 10],
    }

    mod parens {
        #![allow(unused_parens)] // test that type is found even in parentheses

        use derive_more::Display;

        #[derive(Display)]
        struct Paren<T> {
            t: (T),
        }
    }

    #[derive(Display)]
    struct ParenthesizedGenericArgumentsInput<T> {
        t: dyn Fn(T) -> i32,
    }

    #[derive(Display)]
    struct ParenthesizedGenericArgumentsOutput<T> {
        t: dyn Fn(i32) -> T,
    }

    #[derive(Display)]
    struct Ptr<T> {
        t: *const T,
    }

    #[derive(Display)]
    struct Reference<'a, T> {
        t: &'a T,
    }

    #[derive(Display)]
    struct Slice<'a, T> {
        t: &'a [T],
    }

    #[derive(Display)]
    struct BareFn<T> {
        t: Box<fn(T) -> T>,
    }

    #[derive(Display)]
    struct Tuple<T> {
        t: Box<(T, T)>,
    }

    trait MyTrait<T> {}

    #[derive(Display)]
    struct TraitObject<T> {
        t: Box<dyn MyTrait<T>>,
    }

    #[test]
    fn assert() {
        assert_eq!(
            format!(
                "{}",
                ItemStruct {
                    next: OptionalBox {
                        inner: Some(Box::new(ItemStruct {
                            next: OptionalBox { inner: None },
                        })),
                    },
                },
            ),
            "Some(ItemStruct { next: OptionalBox { inner: None } })",
        );

        assert_eq!(
            format!(
                "{}",
                ItemTuple(OptionalBox {
                    inner: Some(Box::new(ItemTuple(OptionalBox { inner: None }))),
                }),
            ),
            "Some(ItemTuple(OptionalBox { inner: None }))",
        );

        assert_eq!(
            format!(
                "{}",
                ItemTupleContainerFmt(OptionalBox {
                    inner: Some(Box::new(ItemTupleContainerFmt(OptionalBox {
                        inner: None,
                    }))),
                }),
            ),
            "Item(Some(ItemTupleContainerFmt(OptionalBox { inner: None })))",
        );

        let item = ItemEnumOuterFormat::Variant1 {
            next: OptionalBox {
                inner: Some(Box::new(ItemEnumOuterFormat::Variant2 {
                    next: OptionalBox { inner: None },
                })),
            },
        };
        assert_eq!(
            format!("{item}"),
            "Some(Variant2 { next: OptionalBox { inner: None } })",
        )
    }
}
