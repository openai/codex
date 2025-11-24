#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
extern crate alloc;

mod structs {
    mod unit {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[derive(Debug)]
        struct Unit;

        #[derive(Debug)]
        struct Tuple();

        #[derive(Debug)]
        struct Struct {}

        #[test]
        fn assert() {
            assert_eq!(format!("{:?}", Unit), "Unit");
            assert_eq!(format!("{:#?}", Unit), "Unit");
            assert_eq!(format!("{:?}", Tuple()), "Tuple");
            assert_eq!(format!("{:#?}", Tuple()), "Tuple");
            assert_eq!(format!("{:?}", Struct {}), "Struct");
            assert_eq!(format!("{:#?}", Struct {}), "Struct");
        }

        mod interpolated_struct {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            #[debug("Format String")]
            struct Unit;

            #[test]
            fn assert() {
                assert_eq!(format!("{Unit:?}"), "Format String");
            }
        }

        mod transparency {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            const I32: i32 = 11;
            const F64: f64 = 3.15;
            const POINTER: &f64 = &3.15;

            #[derive(Debug)]
            #[debug("{I32}")]
            struct Display;

            #[derive(Debug)]
            #[debug("{I32:?}")]
            struct StructDebug;

            #[derive(Debug)]
            #[debug("{:b}", I32)]
            struct Binary;

            #[derive(Debug)]
            #[debug("{0:o}", I32)]
            struct Octal;

            #[derive(Debug)]
            #[debug("{I32:x}")]
            struct LowerHex;

            #[derive(Debug)]
            #[debug("{:X}", I32)]
            struct UpperHex;

            #[derive(Debug)]
            #[debug("{F64:e}")]
            struct LowerExp;

            #[derive(Debug)]
            #[debug("{named:E}", named = F64)]
            struct UpperExp;

            #[derive(Debug)]
            #[debug("{POINTER:p}")]
            struct Pointer;

            #[test]
            fn assert() {
                assert_eq!(format!("{:03?}", Display), "011");
                assert_eq!(format!("{:03?}", StructDebug), "011");
                assert_eq!(format!("{:07?}", Binary), "0001011");
                assert_eq!(format!("{:07?}", Octal), "0000013");
                assert_eq!(format!("{:03?}", LowerHex), "00b");
                assert_eq!(format!("{:03?}", UpperHex), "00B");
                assert_eq!(format!("{:07?}", LowerExp), "03.15e0");
                assert_eq!(format!("{:07?}", UpperExp), "03.15E0");
                assert_eq!(format!("{:018?}", Pointer), format!("{POINTER:018p}"));
            }

            mod omitted {
                mod on_modifiers {
                    #[cfg(not(feature = "std"))]
                    use alloc::format;

                    use derive_more::Debug;

                    const I32: i32 = 11;
                    const F64: f64 = 3.15;

                    #[derive(Debug)]
                    #[debug("{I32:x?}")]
                    struct LowerDebug;

                    #[derive(Debug)]
                    #[debug("{I32:X?}")]
                    struct UpperDebug;

                    #[derive(Debug)]
                    #[debug("{:^}", I32)]
                    struct Align;

                    #[derive(Debug)]
                    #[debug("{:+}", I32)]
                    struct Sign;

                    #[derive(Debug)]
                    #[debug("{:#b}", I32)]
                    struct Alternate;

                    #[derive(Debug)]
                    #[debug("{:0}", I32)]
                    struct ZeroPadded;

                    #[derive(Debug)]
                    #[debug("{:07}", I32)]
                    struct Width;

                    #[derive(Debug)]
                    #[debug("{:.1}", F64)]
                    struct Precision;

                    #[test]
                    fn assert() {
                        assert_eq!(format!("{:03?}", LowerDebug), "b");
                        assert_eq!(format!("{:03?}", UpperDebug), "B");
                        assert_eq!(format!("{:03?}", Align), "11");
                        assert_eq!(format!("{:04?}", Sign), "+11");
                        assert_eq!(format!("{:07?}", Alternate), "0b1011");
                        assert_eq!(format!("{:07?}", ZeroPadded), "11");
                        assert_eq!(format!("{:03?}", Width), "0000011");
                        assert_eq!(format!("{:.3?}", Precision), "3.1");
                    }
                }
            }
        }
    }

    mod single_field {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[derive(Debug)]
        struct Tuple(i32);

        #[derive(Debug)]
        struct Struct {
            field: i32,
        }

        #[test]
        fn assert() {
            assert_eq!(format!("{:?}", Tuple(0)), "Tuple(0)");
            assert_eq!(format!("{:#?}", Tuple(0)), "Tuple(\n    0,\n)");
            assert_eq!(format!("{:?}", Struct { field: 0 }), "Struct { field: 0 }");
            assert_eq!(
                format!("{:#?}", Struct { field: 0 }),
                "Struct {\n    field: 0,\n}",
            );
        }

        mod str_field {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            struct Tuple(#[debug("i32")] i32);

            #[derive(Debug)]
            struct Struct {
                #[debug("i32")]
                field: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:?}", Tuple(0)), "Tuple(i32)");
                assert_eq!(format!("{:#?}", Tuple(0)), "Tuple(\n    i32,\n)");
                assert_eq!(
                    format!("{:?}", Struct { field: 0 }),
                    "Struct { field: i32 }",
                );
                assert_eq!(
                    format!("{:#?}", Struct { field: 0 }),
                    "Struct {\n    field: i32,\n}",
                );
            }
        }

        mod interpolated_field {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            struct Tuple(#[debug("{_0}.{}", _0)] i32);

            #[derive(Debug)]
            struct Struct {
                #[debug("{field}.{}", field)]
                field: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:?}", Tuple(0)), "Tuple(0.0)");
                assert_eq!(format!("{:#?}", Tuple(0)), "Tuple(\n    0.0,\n)");
                assert_eq!(
                    format!("{:?}", Struct { field: 0 }),
                    "Struct { field: 0.0 }",
                );
                assert_eq!(
                    format!("{:#?}", Struct { field: 0 }),
                    "Struct {\n    field: 0.0,\n}",
                );
            }

            mod pointer {
                #[cfg(not(feature = "std"))]
                use alloc::format;

                use derive_more::Debug;

                #[derive(Debug)]
                struct Tuple<'a>(#[debug("{_0:p}.{:p}", self.0)] &'a i32);

                #[derive(Debug)]
                struct Struct<'a> {
                    #[debug("{field:p}.{:p}", self.field)]
                    field: &'a i32,
                }

                #[derive(Debug)]
                #[debug("{_0:p}")]
                struct TupleTransparent<'a>(&'a i32);

                #[derive(Debug)]
                #[debug("{field:p}")]
                struct StructTransparent<'a> {
                    field: &'a i32,
                }

                #[test]
                fn assert() {
                    let a = 42;
                    assert_eq!(
                        format!("{:?}", Tuple(&a)),
                        format!("Tuple({0:p}.{0:p})", &a),
                    );
                    assert_eq!(
                        format!("{:?}", Struct { field: &a }),
                        format!("Struct {{ field: {0:p}.{0:p} }}", &a),
                    );
                    assert_eq!(
                        format!("{:?}", TupleTransparent(&a)),
                        format!("{0:p}", &a),
                    );
                    assert_eq!(
                        format!("{:?}", StructTransparent { field: &a }),
                        format!("{0:p}", &a),
                    );
                }
            }
        }

        mod ignore {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            struct Tuple(#[debug(ignore)] i32);

            #[derive(Debug)]
            struct Struct {
                #[debug(skip)]
                field: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:?}", Tuple(0)), "Tuple(..)");
                assert_eq!(format!("{:#?}", Tuple(0)), "Tuple(..)");
                assert_eq!(format!("{:?}", Struct { field: 0 }), "Struct { .. }");
                assert_eq!(format!("{:#?}", Struct { field: 0 }), "Struct { .. }");
            }
        }

        mod transparency {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            #[debug("{_0:?}")]
            struct TupleDebug(i32);

            #[derive(Debug)]
            #[debug("{}", field)]
            struct StructDisplay {
                field: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:03?}", TupleDebug(7)), "007");
                assert_eq!(format!("{:03?}", StructDisplay { field: 7 }), "007");
            }

            mod suppressed {
                #[cfg(not(feature = "std"))]
                use alloc::format;

                use derive_more::Debug;

                #[derive(Debug)]
                #[debug("{}", format_args!("{_0:?}"))]
                struct TupleDebug(i32);

                #[derive(Debug)]
                #[debug("{}", format_args!("{}", field))]
                struct StructDisplay {
                    field: i32,
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03?}", TupleDebug(7)), "7");
                    assert_eq!(format!("{:03?}", StructDisplay { field: 7 }), "7");
                }
            }

            mod omitted {
                mod on_modifiers {
                    #[cfg(not(feature = "std"))]
                    use alloc::format;

                    use derive_more::Debug;

                    #[derive(Debug)]
                    #[debug("{_0:x?}")]
                    struct LowerDebug(i32);

                    #[derive(Debug)]
                    #[debug("{_0:X?}")]
                    struct UpperDebug(i32);

                    #[derive(Debug)]
                    #[debug("{:^}", _0)]
                    struct Align(i32);

                    #[derive(Debug)]
                    #[debug("{:+}", _0)]
                    struct Sign(i32);

                    #[derive(Debug)]
                    #[debug("{:#b}", _0)]
                    struct Alternate(i32);

                    #[derive(Debug)]
                    #[debug("{:0}", _0)]
                    struct ZeroPadded(i32);

                    #[derive(Debug)]
                    #[debug("{:07}", _0)]
                    struct Width(i32);

                    #[derive(Debug)]
                    #[debug("{:.5}", _0)]
                    struct Precision(f64);

                    #[test]
                    fn assert() {
                        assert_eq!(format!("{:03?}", LowerDebug(7)), "7");
                        assert_eq!(format!("{:03?}", UpperDebug(8)), "8");
                        assert_eq!(format!("{:03?}", Align(5)), "5");
                        assert_eq!(format!("{:03?}", Sign(5)), "+5");
                        assert_eq!(format!("{:07?}", Alternate(5)), "0b101");
                        assert_eq!(format!("{:07?}", ZeroPadded(-5)), "-5");
                        assert_eq!(format!("{:03?}", Width(5)), "0000005");
                        assert_eq!(format!("{:.3?}", Precision(1.23456789)), "1.23457");
                    }
                }
            }
        }
    }

    mod multi_field {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[derive(Debug)]
        struct Tuple(i32, i32);

        #[derive(Debug)]
        struct Struct {
            field1: i32,
            field2: i32,
        }

        #[test]
        fn assert() {
            assert_eq!(format!("{:?}", Tuple(1, 2)), "Tuple(1, 2)");
            assert_eq!(format!("{:#?}", Tuple(1, 2)), "Tuple(\n    1,\n    2,\n)");
            assert_eq!(
                format!(
                    "{:?}",
                    Struct {
                        field1: 1,
                        field2: 2,
                    }
                ),
                "Struct { field1: 1, field2: 2 }",
            );
            assert_eq!(
                format!(
                    "{:#?}",
                    Struct {
                        field1: 1,
                        field2: 2,
                    }
                ),
                "Struct {\n    field1: 1,\n    field2: 2,\n}",
            );
        }

        mod str_field {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            struct Tuple(i32, #[debug("i32")] i32);

            #[derive(Debug)]
            struct Struct {
                #[debug("i32")]
                field1: i32,
                field2: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:?}", Tuple(1, 2)), "Tuple(1, i32)");
                assert_eq!(
                    format!("{:#?}", Tuple(1, 2)),
                    "Tuple(\n    1,\n    i32,\n)",
                );
                assert_eq!(
                    format!(
                        "{:?}",
                        Struct {
                            field1: 1,
                            field2: 2,
                        }
                    ),
                    "Struct { field1: i32, field2: 2 }",
                );
                assert_eq!(
                    format!(
                        "{:#?}",
                        Struct {
                            field1: 1,
                            field2: 2,
                        }
                    ),
                    "Struct {\n    field1: i32,\n    field2: 2,\n}",
                );
            }
        }

        mod interpolated_field {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            struct Tuple(i32, #[debug("{_0}.{}", _1)] i32);

            #[derive(Debug)]
            struct Struct {
                #[debug("{field1}.{}", field2)]
                field1: i32,
                field2: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:?}", Tuple(1, 2)), "Tuple(1, 1.2)");
                assert_eq!(
                    format!("{:#?}", Tuple(1, 2)),
                    "Tuple(\n    1,\n    1.2,\n)",
                );
                assert_eq!(
                    format!(
                        "{:?}",
                        Struct {
                            field1: 1,
                            field2: 2,
                        }
                    ),
                    "Struct { field1: 1.2, field2: 2 }",
                );
                assert_eq!(
                    format!(
                        "{:#?}",
                        Struct {
                            field1: 1,
                            field2: 2,
                        }
                    ),
                    "Struct {\n    field1: 1.2,\n    field2: 2,\n}",
                );
            }
        }

        mod interpolated_struct {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            #[debug("{_0} * {_1}")]
            struct Tuple(u8, bool);

            #[derive(Debug)]
            #[debug("{a} * {b}")]
            struct Struct {
                a: u8,
                b: bool,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:?}", Tuple(10, true)), "10 * true");
                assert_eq!(format!("{:?}", Struct { a: 10, b: true }), "10 * true");
            }

            mod pointer {
                #[cfg(not(feature = "std"))]
                use alloc::format;

                use derive_more::Debug;

                #[derive(Debug)]
                #[debug("{_0:p} * {_1:p}")]
                struct Tuple<'a, 'b>(&'a u8, &'b bool);

                #[derive(Debug)]
                #[debug("{a:p} * {b:p}")]
                struct Struct<'a, 'b> {
                    a: &'a u8,
                    b: &'b bool,
                }

                #[test]
                fn assert() {
                    let (a, b) = (10, true);
                    assert_eq!(
                        format!("{:?}", Tuple(&a, &b)),
                        format!("{:p} * {:p}", &a, &b),
                    );
                    assert_eq!(
                        format!("{:?}", Struct { a: &a, b: &b }),
                        format!("{:p} * {:p}", &a, &b),
                    );
                }
            }
        }

        mod ignore {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            struct Tuple(#[debug(ignore)] i32, i32);

            #[derive(Debug)]
            struct Struct {
                field1: i32,
                #[debug(skip)]
                field2: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:?}", Tuple(1, 2)), "Tuple(2, ..)");
                assert_eq!(format!("{:#?}", Tuple(1, 2)), "Tuple(\n    2,\n    ..\n)",);
                assert_eq!(
                    format!(
                        "{:?}",
                        Struct {
                            field1: 1,
                            field2: 2
                        }
                    ),
                    "Struct { field1: 1, .. }",
                );
                assert_eq!(
                    format!(
                        "{:#?}",
                        Struct {
                            field1: 1,
                            field2: 2
                        }
                    ),
                    "Struct {\n    field1: 1,\n    ..\n}",
                );
            }
        }

        mod transparency {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            #[debug("{0:o}", _0)]
            struct TupleOctal(i32, i64);

            #[derive(Debug)]
            #[debug("{named:e}", named = b)]
            struct StructLowerExp {
                a: i32,
                b: f64,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:03?}", TupleOctal(9, 4)), "011");
                assert_eq!(
                    format!("{:.1?}", StructLowerExp { a: 7, b: 3.15 }),
                    "3.1e0",
                );
            }
        }
    }
}

mod enums {
    mod no_variants {
        use derive_more::Debug;

        #[derive(Debug)]
        enum Void {}

        const fn assert<T: core::fmt::Debug>() {}
        const _: () = assert::<Void>();
    }

    mod unit_variant {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[derive(Debug)]
        enum Enum {
            Unit,
            Unnamed(),
            Named {},
        }

        #[test]
        fn assert() {
            assert_eq!(format!("{:?}", Enum::Unit), "Unit");
            assert_eq!(format!("{:#?}", Enum::Unit), "Unit");
            assert_eq!(format!("{:?}", Enum::Unnamed()), "Unnamed");
            assert_eq!(format!("{:#?}", Enum::Unnamed()), "Unnamed");
            assert_eq!(format!("{:?}", Enum::Named {}), "Named");
            assert_eq!(format!("{:#?}", Enum::Named {}), "Named");
        }

        mod transparency {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            const I32: i32 = 11;
            const F64: f64 = 3.15;
            const POINTER: &f64 = &3.15;

            #[derive(Debug)]
            enum Unit {
                #[debug("{I32}")]
                Display,
                #[debug("{I32:?}")]
                Debug,
                #[debug("{:b}", I32)]
                Binary,
                #[debug("{0:o}", I32)]
                Octal,
                #[debug("{I32:x}")]
                LowerHex,
                #[debug("{:X}", I32)]
                UpperHex,
                #[debug("{F64:e}")]
                LowerExp,
                #[debug("{named:E}", named = F64)]
                UpperExp,
                #[debug("{POINTER:p}")]
                Pointer,
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:03?}", Unit::Display), "011");
                assert_eq!(format!("{:03?}", Unit::Debug), "011");
                assert_eq!(format!("{:07?}", Unit::Binary), "0001011");
                assert_eq!(format!("{:07?}", Unit::Octal), "0000013");
                assert_eq!(format!("{:03?}", Unit::LowerHex), "00b");
                assert_eq!(format!("{:03?}", Unit::UpperHex), "00B");
                assert_eq!(format!("{:07?}", Unit::LowerExp), "03.15e0");
                assert_eq!(format!("{:07?}", Unit::UpperExp), "03.15E0");
                assert_eq!(
                    format!("{:018?}", Unit::Pointer),
                    format!("{POINTER:018p}"),
                );
            }

            mod omitted {
                mod on_modifiers {
                    #[cfg(not(feature = "std"))]
                    use alloc::format;

                    use derive_more::Debug;

                    const I32: i32 = 11;
                    const F64: f64 = 3.15;

                    #[derive(Debug)]
                    enum Unit {
                        #[debug("{I32:x?}")]
                        LowerDebug,
                        #[debug("{I32:X?}")]
                        UpperDebug,
                        #[debug("{:^}", I32)]
                        Align,
                        #[debug("{:+}", I32)]
                        Sign,
                        #[debug("{:#b}", I32)]
                        Alternate,
                        #[debug("{:0}", I32)]
                        ZeroPadded,
                        #[debug("{:07}", I32)]
                        Width,
                        #[debug("{:.1}", F64)]
                        Precision,
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(format!("{:03?}", Unit::LowerDebug), "b");
                        assert_eq!(format!("{:03?}", Unit::UpperDebug), "B");
                        assert_eq!(format!("{:03?}", Unit::Align), "11");
                        assert_eq!(format!("{:04?}", Unit::Sign), "+11");
                        assert_eq!(format!("{:07?}", Unit::Alternate), "0b1011");
                        assert_eq!(format!("{:07?}", Unit::ZeroPadded), "11");
                        assert_eq!(format!("{:03?}", Unit::Width), "0000011");
                        assert_eq!(format!("{:.3?}", Unit::Precision), "3.1");
                    }
                }
            }
        }
    }

    mod single_field_variant {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[derive(Debug)]
        enum Enum {
            Unnamed(i32),
            Named {
                field: i32,
            },
            StrUnnamed(#[debug("i32")] i32),
            StrNamed {
                #[debug("i32")]
                field: i32,
            },
            InterpolatedUnnamed(#[debug("{_0}.{}", _0)] i32),
            InterpolatedNamed {
                #[debug("{field}.{}", field)]
                field: i32,
            },
            SkippedUnnamed(#[debug(skip)] i32),
            SkippedNamed {
                #[debug(skip)]
                field: i32,
            },
        }

        #[test]
        fn assert() {
            assert_eq!(format!("{:?}", Enum::Unnamed(1)), "Unnamed(1)");
            assert_eq!(format!("{:#?}", Enum::Unnamed(1)), "Unnamed(\n    1,\n)");
            assert_eq!(
                format!("{:?}", Enum::Named { field: 1 }),
                "Named { field: 1 }",
            );
            assert_eq!(
                format!("{:#?}", Enum::Named { field: 1 }),
                "Named {\n    field: 1,\n}",
            );
            assert_eq!(format!("{:?}", Enum::StrUnnamed(1)), "StrUnnamed(i32)");
            assert_eq!(
                format!("{:#?}", Enum::StrUnnamed(1)),
                "StrUnnamed(\n    i32,\n)",
            );
            assert_eq!(
                format!("{:?}", Enum::StrNamed { field: 1 }),
                "StrNamed { field: i32 }",
            );
            assert_eq!(
                format!("{:#?}", Enum::StrNamed { field: 1 }),
                "StrNamed {\n    field: i32,\n}",
            );
            assert_eq!(
                format!("{:?}", Enum::InterpolatedUnnamed(1)),
                "InterpolatedUnnamed(1.1)",
            );
            assert_eq!(
                format!("{:#?}", Enum::InterpolatedUnnamed(1)),
                "InterpolatedUnnamed(\n    1.1,\n)",
            );
            assert_eq!(
                format!("{:?}", Enum::InterpolatedNamed { field: 1 }),
                "InterpolatedNamed { field: 1.1 }",
            );
            assert_eq!(
                format!("{:#?}", Enum::InterpolatedNamed { field: 1 }),
                "InterpolatedNamed {\n    field: 1.1,\n}",
            );
            assert_eq!(
                format!("{:?}", Enum::SkippedUnnamed(1)),
                "SkippedUnnamed(..)",
            );
            assert_eq!(
                format!("{:#?}", Enum::SkippedUnnamed(1)),
                "SkippedUnnamed(..)",
            );
            assert_eq!(
                format!("{:?}", Enum::SkippedNamed { field: 1 }),
                "SkippedNamed { .. }",
            );
            assert_eq!(
                format!("{:#?}", Enum::SkippedNamed { field: 1 }),
                "SkippedNamed { .. }",
            );
        }

        mod transparency {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            enum Enum {
                #[debug("{_0:?}")]
                Debug(i32),
                #[debug("{}", field)]
                Display { field: i32 },
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:03?}", Enum::Debug(7)), "007");
                assert_eq!(format!("{:03?}", Enum::Display { field: 7 }), "007");
            }

            mod suppressed {
                #[cfg(not(feature = "std"))]
                use alloc::format;

                use derive_more::Debug;

                #[derive(Debug)]
                enum Enum {
                    #[debug("{}", format_args!("{_0:?}"))]
                    Debug(i32),
                    #[debug("{}", format_args!("{}", field))]
                    Display { field: i32 },
                }

                #[test]
                fn assert() {
                    assert_eq!(format!("{:03?}", Enum::Debug(7)), "7");
                    assert_eq!(format!("{:03?}", Enum::Display { field: 7 }), "7");
                }
            }

            mod omitted {
                mod on_modifiers {
                    #[cfg(not(feature = "std"))]
                    use alloc::format;

                    use derive_more::Debug;

                    #[derive(Debug)]
                    enum Enum {
                        #[debug("{_0:x?}")]
                        LowerDebug(i32),
                        #[debug("{_0:X?}")]
                        UpperDebug(i32),
                        #[debug("{:^}", _0)]
                        Align(i32),
                        #[debug("{:+}", _0)]
                        Sign(i32),
                        #[debug("{:#b}", _0)]
                        Alternate(i32),
                        #[debug("{:0}", _0)]
                        ZeroPadded(i32),
                        #[debug("{:07}", _0)]
                        Width(i32),
                        #[debug("{:.5}", _0)]
                        Precision(f64),
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(format!("{:03?}", Enum::LowerDebug(7)), "7");
                        assert_eq!(format!("{:03?}", Enum::UpperDebug(8)), "8");
                        assert_eq!(format!("{:03?}", Enum::Align(5)), "5");
                        assert_eq!(format!("{:03?}", Enum::Sign(5)), "+5");
                        assert_eq!(format!("{:07?}", Enum::Alternate(5)), "0b101");
                        assert_eq!(format!("{:07?}", Enum::ZeroPadded(-5)), "-5");
                        assert_eq!(format!("{:03?}", Enum::Width(5)), "0000005");
                        assert_eq!(
                            format!("{:.3?}", Enum::Precision(1.23456789)),
                            "1.23457",
                        );
                    }
                }
            }
        }
    }

    mod multi_field_variant {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[derive(Debug)]
        enum Enum {
            Unnamed(i32, i32),
            Named {
                field1: i32,
                field2: i32,
            },
            StrUnnamed(#[debug("i32")] i32, i32),
            StrNamed {
                field1: i32,
                #[debug("i32")]
                field2: i32,
            },
            InterpolatedUnnamed(i32, #[debug("{_0}.{}", _1)] i32),
            InterpolatedNamed {
                #[debug("{field1}.{}", field2)]
                field1: i32,
                field2: i32,
            },
            SkippedUnnamed(i32, #[debug(skip)] i32),
            SkippedNamed {
                #[debug(skip)]
                field1: i32,
                field2: i32,
            },
        }

        #[test]
        fn assert() {
            assert_eq!(format!("{:?}", Enum::Unnamed(1, 2)), "Unnamed(1, 2)");
            assert_eq!(
                format!("{:#?}", Enum::Unnamed(1, 2)),
                "Unnamed(\n    1,\n    2,\n)",
            );
            assert_eq!(
                format!("{:?}", Enum::StrUnnamed(1, 2)),
                "StrUnnamed(i32, 2)",
            );
            assert_eq!(
                format!("{:#?}", Enum::StrUnnamed(1, 2)),
                "StrUnnamed(\n    i32,\n    2,\n)",
            );
            assert_eq!(
                format!(
                    "{:?}",
                    Enum::StrNamed {
                        field1: 1,
                        field2: 2,
                    }
                ),
                "StrNamed { field1: 1, field2: i32 }",
            );
            assert_eq!(
                format!(
                    "{:#?}",
                    Enum::StrNamed {
                        field1: 1,
                        field2: 2,
                    }
                ),
                "StrNamed {\n    field1: 1,\n    field2: i32,\n}",
            );
            assert_eq!(
                format!("{:?}", Enum::InterpolatedUnnamed(1, 2)),
                "InterpolatedUnnamed(1, 1.2)",
            );
            assert_eq!(
                format!("{:#?}", Enum::InterpolatedUnnamed(1, 2)),
                "InterpolatedUnnamed(\n    1,\n    1.2,\n)",
            );
            assert_eq!(
                format!(
                    "{:?}",
                    Enum::InterpolatedNamed {
                        field1: 1,
                        field2: 2,
                    }
                ),
                "InterpolatedNamed { field1: 1.2, field2: 2 }",
            );
            assert_eq!(
                format!(
                    "{:#?}",
                    Enum::InterpolatedNamed {
                        field1: 1,
                        field2: 2,
                    }
                ),
                "InterpolatedNamed {\n    field1: 1.2,\n    field2: 2,\n}",
            );
            assert_eq!(
                format!("{:?}", Enum::SkippedUnnamed(1, 2)),
                "SkippedUnnamed(1, ..)",
            );
            assert_eq!(
                format!("{:#?}", Enum::SkippedUnnamed(1, 2)),
                "SkippedUnnamed(\n    1,\n    ..\n)",
            );
            assert_eq!(
                format!(
                    "{:?}",
                    Enum::SkippedNamed {
                        field1: 1,
                        field2: 2,
                    }
                ),
                "SkippedNamed { field2: 2, .. }",
            );
            assert_eq!(
                format!(
                    "{:#?}",
                    Enum::SkippedNamed {
                        field1: 1,
                        field2: 2,
                    }
                ),
                "SkippedNamed {\n    field2: 2,\n    ..\n}",
            );
        }

        mod interpolated_variant {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            enum Enum {
                #[debug("Format String")]
                Unit,
                #[debug("Format {a} String {b}")]
                Fields { a: usize, b: u8 },
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:?}", Enum::Unit), "Format String");
                assert_eq!(
                    format!("{:?}", Enum::Fields { a: 1, b: 2 }),
                    "Format 1 String 2",
                );
            }
        }

        mod transparency {
            #[cfg(not(feature = "std"))]
            use alloc::format;

            use derive_more::Debug;

            #[derive(Debug)]
            enum Enum {
                #[debug("{0:o}", _0)]
                TupleOctal(i32, i64),
                #[debug("{named:e}", named = b)]
                StructLowerExp { a: i32, b: f64 },
            }

            #[test]
            fn assert() {
                assert_eq!(format!("{:03?}", Enum::TupleOctal(9, 4)), "011");
                assert_eq!(
                    format!("{:.1?}", Enum::StructLowerExp { a: 7, b: 3.15 }),
                    "3.1e0",
                );
            }
        }
    }
}

mod generic {
    #[cfg(not(feature = "std"))]
    use alloc::{boxed::Box, format};
    use core::fmt;

    use derive_more::Debug;

    struct NotDebug;

    impl fmt::Display for NotDebug {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_tuple("NotDebug").finish()
        }
    }

    #[derive(Debug)]
    struct NamedGenericStruct<T> {
        field: T,
    }
    #[test]
    fn named_generic_struct() {
        assert_eq!(
            format!("{:?}", NamedGenericStruct { field: 1 }),
            "NamedGenericStruct { field: 1 }",
        );
        assert_eq!(
            format!("{:#?}", NamedGenericStruct { field: 1 }),
            "NamedGenericStruct {\n    field: 1,\n}",
        );
    }

    #[derive(Debug)]
    struct NamedGenericStructUnsized<T: ?Sized> {
        field: T,
    }
    #[test]
    fn named_generic_struct_unsized() {
        assert_eq!(
            format!("{:?}", NamedGenericStructUnsized { field: 1 }),
            "NamedGenericStructUnsized { field: 1 }",
        );
        assert_eq!(
            format!("{:#?}", NamedGenericStructUnsized { field: 1 }),
            "NamedGenericStructUnsized {\n    field: 1,\n}",
        );
    }

    #[derive(Debug)]
    struct NamedGenericStructIgnored<T> {
        #[debug(ignore)]
        field: T,
    }
    #[test]
    fn named_generic_struct_ignored() {
        assert_eq!(
            format!("{:?}", NamedGenericStructIgnored { field: NotDebug }),
            "NamedGenericStructIgnored { .. }",
        );
        assert_eq!(
            format!("{:#?}", NamedGenericStructIgnored { field: NotDebug }),
            "NamedGenericStructIgnored { .. }",
        );
    }

    #[derive(Debug)]
    struct InterpolatedNamedGenericStruct<T> {
        #[debug("{field}.{}", field)]
        field: T,
    }
    #[test]
    fn interpolated_named_generic_struct() {
        assert_eq!(
            format!("{:?}", InterpolatedNamedGenericStruct { field: 1 }),
            "InterpolatedNamedGenericStruct { field: 1.1 }",
        );
        assert_eq!(
            format!("{:#?}", InterpolatedNamedGenericStruct { field: 1 }),
            "InterpolatedNamedGenericStruct {\n    field: 1.1,\n}",
        );
        assert_eq!(
            format!("{:?}", InterpolatedNamedGenericStruct { field: NotDebug }),
            "InterpolatedNamedGenericStruct { field: NotDebug.NotDebug }",
        );
        assert_eq!(
            format!("{:#?}", InterpolatedNamedGenericStruct { field: NotDebug }),
            "InterpolatedNamedGenericStruct {\n    field: NotDebug.NotDebug,\n}",
        );
    }

    #[derive(Debug)]
    struct InterpolatedNamedGenericStructWidthPrecision<T> {
        #[debug("{field:<>width$.prec$}.{field}")]
        field: T,
        width: usize,
        prec: usize,
    }
    #[test]
    fn interpolated_named_generic_struct_width_precision() {
        assert_eq!(
            format!(
                "{:?}",
                InterpolatedNamedGenericStructWidthPrecision {
                    field: 1.2345,
                    width: 9,
                    prec: 2,
                }
            ),
            "InterpolatedNamedGenericStructWidthPrecision { \
                    field: <<<<<1.23.1.2345, \
                    width: 9, \
                    prec: 2 \
                }",
        );
        assert_eq!(
            format!(
                "{:#?}",
                InterpolatedNamedGenericStructWidthPrecision {
                    field: 1.2345,
                    width: 9,
                    prec: 2,
                }
            ),
            "InterpolatedNamedGenericStructWidthPrecision {\n    \
                    field: <<<<<1.23.1.2345,\n    \
                    width: 9,\n    \
                    prec: 2,\n\
                }",
        );
    }

    #[derive(Debug)]
    #[debug("test_named")]
    struct InterpolatedNamedGenericStructIgnored<T> {
        field: T,
    }
    #[test]
    fn interpolated_named_generic_struct_ignored() {
        assert_eq!(
            format!(
                "{:?}",
                InterpolatedNamedGenericStructIgnored { field: NotDebug },
            ),
            "test_named",
        );
    }

    #[derive(Debug)]
    struct AliasedNamedGenericStruct<T> {
        #[debug("{alias}", alias = field)]
        field: T,
    }
    #[test]
    fn aliased_named_generic_struct() {
        assert_eq!(
            format!("{:?}", AliasedNamedGenericStruct { field: 1 }),
            "AliasedNamedGenericStruct { field: 1 }",
        );
        assert_eq!(
            format!("{:#?}", AliasedNamedGenericStruct { field: 1 }),
            "AliasedNamedGenericStruct {\n    field: 1,\n}",
        );
    }

    #[derive(Debug)]
    struct AliasedFieldNamedGenericStruct<T> {
        #[debug("{field1}", field1 = field2)]
        field1: T,
        field2: i32,
    }
    #[test]
    fn aliased_field_named_generic_struct() {
        assert_eq!(
            format!(
                "{:?}",
                AliasedFieldNamedGenericStruct {
                    field1: NotDebug,
                    field2: 1,
                },
            ),
            "AliasedFieldNamedGenericStruct { field1: 1, field2: 1 }",
        );
        assert_eq!(
            format!(
                "{:#?}",
                AliasedFieldNamedGenericStruct {
                    field1: NotDebug,
                    field2: 1,
                },
            ),
            "AliasedFieldNamedGenericStruct {\n    field1: 1,\n    field2: 1,\n}",
        );
    }

    #[derive(Debug)]
    struct UnnamedGenericStruct<T>(T);
    #[test]
    fn unnamed_generic_struct() {
        assert_eq!(
            format!("{:?}", UnnamedGenericStruct(2)),
            "UnnamedGenericStruct(2)",
        );
        assert_eq!(
            format!("{:#?}", UnnamedGenericStruct(2)),
            "UnnamedGenericStruct(\n    2,\n)",
        );
    }

    #[derive(Debug)]
    struct UnnamedGenericStructUnsized<T: ?Sized>(T);
    #[test]
    fn unnamed_generic_struct_unsized() {
        assert_eq!(
            format!("{:?}", UnnamedGenericStructUnsized(2)),
            "UnnamedGenericStructUnsized(2)",
        );
        assert_eq!(
            format!("{:#?}", UnnamedGenericStructUnsized(2)),
            "UnnamedGenericStructUnsized(\n    2,\n)",
        );
    }

    #[derive(Debug)]
    struct UnnamedGenericStructIgnored<T>(#[debug(skip)] T);
    #[test]
    fn unnamed_generic_struct_ignored() {
        assert_eq!(
            format!("{:?}", UnnamedGenericStructIgnored(NotDebug)),
            "UnnamedGenericStructIgnored(..)",
        );
        assert_eq!(
            format!("{:#?}", UnnamedGenericStructIgnored(NotDebug)),
            "UnnamedGenericStructIgnored(..)",
        );
    }

    #[derive(Debug)]
    struct InterpolatedUnnamedGenericStruct<T>(#[debug("{}.{_0}", _0)] T);
    #[test]
    fn interpolated_unnamed_generic_struct() {
        assert_eq!(
            format!("{:?}", InterpolatedUnnamedGenericStruct(2)),
            "InterpolatedUnnamedGenericStruct(2.2)",
        );
        assert_eq!(
            format!("{:#?}", InterpolatedUnnamedGenericStruct(2)),
            "InterpolatedUnnamedGenericStruct(\n    2.2,\n)",
        );
        assert_eq!(
            format!("{:?}", InterpolatedUnnamedGenericStruct(NotDebug)),
            "InterpolatedUnnamedGenericStruct(NotDebug.NotDebug)",
        );
        assert_eq!(
            format!("{:#?}", InterpolatedUnnamedGenericStruct(NotDebug)),
            "InterpolatedUnnamedGenericStruct(\n    NotDebug.NotDebug,\n)",
        );
    }

    #[derive(Debug)]
    #[debug("test_unnamed")]
    struct InterpolatedUnnamedGenericStructIgnored<T>(T);
    #[test]
    fn interpolated_unnamed_generic_struct_ignored() {
        assert_eq!(
            format!("{:?}", InterpolatedUnnamedGenericStructIgnored(NotDebug)),
            "test_unnamed",
        );
    }

    #[derive(Debug)]
    struct AliasedUnnamedGenericStruct<T>(#[debug("{alias}", alias = _0)] T);
    #[test]
    fn aliased_unnamed_generic_struct() {
        assert_eq!(
            format!("{:?}", AliasedUnnamedGenericStruct(2)),
            "AliasedUnnamedGenericStruct(2)",
        );
        assert_eq!(
            format!("{:#?}", AliasedUnnamedGenericStruct(2)),
            "AliasedUnnamedGenericStruct(\n    2,\n)",
        );
    }

    #[derive(Debug)]
    struct AliasedFieldUnnamedGenericStruct<T>(#[debug("{_0}", _0 = _1)] T, i32);
    #[test]
    fn aliased_field_unnamed_generic_struct() {
        assert_eq!(
            format!("{:?}", AliasedFieldUnnamedGenericStruct(NotDebug, 2)),
            "AliasedFieldUnnamedGenericStruct(2, 2)",
        );
        assert_eq!(
            format!("{:#?}", AliasedFieldUnnamedGenericStruct(NotDebug, 2)),
            "AliasedFieldUnnamedGenericStruct(\n    2,\n    2,\n)",
        );
    }

    #[derive(Debug)]
    enum GenericEnum<A, B> {
        A { field: A },
        B(B),
    }
    #[test]
    fn generic_enum() {
        assert_eq!(
            format!("{:?}", GenericEnum::A::<_, u8> { field: 1 }),
            "A { field: 1 }",
        );
        assert_eq!(
            format!("{:#?}", GenericEnum::A::<_, u8> { field: 1 }),
            "A {\n    field: 1,\n}",
        );
        assert_eq!(format!("{:?}", GenericEnum::B::<u8, _>(2)), "B(2)");
        assert_eq!(
            format!("{:#?}", GenericEnum::B::<u8, _>(2)),
            "B(\n    2,\n)",
        );
    }

    #[derive(derive_more::Debug)]
    enum GenericEnumUnsized<A: ?Sized, B: ?Sized + 'static> {
        A { field: Box<A> },
        B(&'static B),
    }
    #[test]
    fn generic_enum_unsized() {
        assert_eq!(
            format!("{:?}", GenericEnumUnsized::A::<i32, u8> { field: 1.into() }),
            "A { field: 1 }",
        );
        assert_eq!(
            format!(
                "{:#?}",
                GenericEnumUnsized::A::<i32, u8> { field: 1.into() },
            ),
            "A {\n    field: 1,\n}",
        );
        assert_eq!(
            format!("{:?}", GenericEnumUnsized::B::<u8, i32>(&2)),
            "B(2)",
        );
        assert_eq!(
            format!("{:#?}", GenericEnumUnsized::B::<u8, i32>(&2)),
            "B(\n    2,\n)",
        );
    }

    #[derive(Debug)]
    enum InterpolatedGenericEnum<A, B> {
        A {
            #[debug("{}.{field}", field)]
            field: A,
        },
        B(#[debug("{}.{_0}", _0)] B),
    }
    #[test]
    fn interpolated_generic_enum() {
        assert_eq!(
            format!("{:?}", InterpolatedGenericEnum::A::<_, u8> { field: 1 }),
            "A { field: 1.1 }",
        );
        assert_eq!(
            format!("{:#?}", InterpolatedGenericEnum::A::<_, u8> { field: 1 }),
            "A {\n    field: 1.1,\n}",
        );
        assert_eq!(
            format!("{:?}", InterpolatedGenericEnum::B::<u8, _>(2)),
            "B(2.2)",
        );
        assert_eq!(
            format!("{:#?}", InterpolatedGenericEnum::B::<u8, _>(2)),
            "B(\n    2.2,\n)",
        );
    }

    #[derive(Debug)]
    enum InterpolatedGenericEnumIgnored<A, B> {
        #[debug("A {field}")]
        A { field: A },
        #[debug("B")]
        B(B),
    }
    #[test]
    fn interpolated_generic_enum_ignored() {
        assert_eq!(
            format!(
                "{:?}",
                InterpolatedGenericEnumIgnored::A::<_, u8> { field: NotDebug },
            ),
            "A NotDebug",
        );
        assert_eq!(
            format!("{:?}", InterpolatedGenericEnumIgnored::B::<u8, _>(NotDebug)),
            "B",
        );
    }

    #[derive(Debug)]
    struct MultiTraitNamedGenericStruct<A, B> {
        #[debug("{}.{}<->{0:o}.{1:#x}<->{0:?}.{1:X?}", a, b)]
        a: A,
        b: B,
    }
    #[test]
    fn multi_trait_named_generic_struct() {
        let s = MultiTraitNamedGenericStruct { a: 8u8, b: 255 };
        assert_eq!(
            format!("{s:?}"),
            "MultiTraitNamedGenericStruct { a: 8.255<->10.0xff<->8.FF, b: 255 }",
        );
        assert_eq!(
                format!("{s:#?}"),
                "MultiTraitNamedGenericStruct {\n    a: 8.255<->10.0xff<->8.FF,\n    b: 255,\n}",
            );
    }

    #[derive(Debug)]
    struct MultiTraitUnnamedGenericStruct<A, B>(
        #[debug("{}.{}.{{}}.{0:o}.{1:#x}-{0:>4?}.{1:^4X?}", _0, _1)] A,
        B,
    );
    #[test]
    fn multi_trait_unnamed_generic_struct() {
        let s = MultiTraitUnnamedGenericStruct(8u8, 255);
        assert_eq!(
            format!("{s:?}"),
            "MultiTraitUnnamedGenericStruct(8.255.{}.10.0xff-   8. FF , 255)",
        );
        assert_eq!(
                format!("{s:#?}"),
                "MultiTraitUnnamedGenericStruct(\n    8.255.{}.10.0xff-   8. FF ,\n    255,\n)",
            );
    }

    #[derive(Debug)]
    struct UnusedGenericStruct<T>(#[debug("{}", 3 * 4)] T);
    #[test]
    fn unused_generic_struct() {
        let s = UnusedGenericStruct(NotDebug);
        assert_eq!(format!("{s:?}"), "UnusedGenericStruct(12)");
        assert_eq!(format!("{s:#?}"), "UnusedGenericStruct(\n    12,\n)");
    }

    mod associated_type_field_enumerator {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        trait Trait {
            type Type;
        }

        struct Struct;

        impl Trait for Struct {
            type Type = i32;
        }

        #[test]
        fn auto_generic_named_struct_associated() {
            #[derive(Debug)]
            struct AutoGenericNamedStructAssociated<T: Trait> {
                field: <T as Trait>::Type,
            }

            let s = AutoGenericNamedStructAssociated::<Struct> { field: 10 };
            assert_eq!(
                format!("{s:?}"),
                "AutoGenericNamedStructAssociated { field: 10 }",
            );
            assert_eq!(
                format!("{s:#?}"),
                "AutoGenericNamedStructAssociated {\n    field: 10,\n}",
            );
        }

        #[test]
        fn auto_generic_unnamed_struct_associated() {
            #[derive(Debug)]
            struct AutoGenericUnnamedStructAssociated<T: Trait>(<T as Trait>::Type);

            let s = AutoGenericUnnamedStructAssociated::<Struct>(10);
            assert_eq!(format!("{s:?}"), "AutoGenericUnnamedStructAssociated(10)",);
            assert_eq!(
                format!("{s:#?}"),
                "AutoGenericUnnamedStructAssociated(\n    10,\n)",
            );
        }

        #[test]
        fn auto_generic_enum_associated() {
            #[derive(Debug)]
            enum AutoGenericEnumAssociated<T: Trait> {
                Enumerator(<T as Trait>::Type),
            }

            let e = AutoGenericEnumAssociated::<Struct>::Enumerator(10);
            assert_eq!(format!("{:?}", e), "Enumerator(10)");
            assert_eq!(format!("{:#?}", e), "Enumerator(\n    10,\n)");
        }
    }

    mod complex_type_field_enumerator {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[derive(Debug)]
        struct Struct<T>(T);

        #[test]
        fn auto_generic_named_struct_complex() {
            #[derive(Debug)]
            struct AutoGenericNamedStructComplex<T> {
                field: Struct<T>,
            }

            let s = AutoGenericNamedStructComplex { field: Struct(10) };
            assert_eq!(
                format!("{s:?}"),
                "AutoGenericNamedStructComplex { field: Struct(10) }",
            );
            assert_eq!(
                    format!("{s:#?}"),
                    "AutoGenericNamedStructComplex {\n    field: Struct(\n        10,\n    ),\n}",
                );
        }

        #[test]
        fn auto_generic_unnamed_struct_complex() {
            #[derive(Debug)]
            struct AutoGenericUnnamedStructComplex<T>(Struct<T>);

            let s = AutoGenericUnnamedStructComplex(Struct(10));
            assert_eq!(
                format!("{s:?}"),
                "AutoGenericUnnamedStructComplex(Struct(10))",
            );
            assert_eq!(
                format!("{s:#?}"),
                "AutoGenericUnnamedStructComplex(\n    Struct(\n        10,\n    ),\n)",
            );
        }

        #[test]
        fn auto_generic_enum_complex() {
            #[derive(Debug)]
            enum AutoGenericEnumComplex<T> {
                Enumerator(Struct<T>),
            }

            let e = AutoGenericEnumComplex::Enumerator(Struct(10));
            assert_eq!(format!("{:?}", e), "Enumerator(Struct(10))");
            assert_eq!(
                format!("{:#?}", e),
                "Enumerator(\n    Struct(\n        10,\n    ),\n)",
            )
        }
    }

    mod reference {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[test]
        fn auto_generic_reference() {
            #[derive(Debug)]
            struct AutoGenericReference<'a, T>(&'a T);

            let s = AutoGenericReference(&10);
            assert_eq!(format!("{s:?}"), "AutoGenericReference(10)");
            assert_eq!(format!("{s:#?}"), "AutoGenericReference(\n    10,\n)");
        }

        #[test]
        fn auto_generic_static_reference() {
            #[derive(Debug)]
            struct AutoGenericStaticReference<T: 'static>(&'static T);

            let s = AutoGenericStaticReference(&10);
            assert_eq!(format!("{s:?}"), "AutoGenericStaticReference(10)");
            assert_eq!(format!("{s:#?}"), "AutoGenericStaticReference(\n    10,\n)",);
        }
    }

    mod indirect {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[derive(Debug)]
        struct Struct<T>(T);

        #[test]
        fn auto_generic_indirect() {
            #[derive(Debug)]
            struct AutoGenericIndirect<T: 'static>(Struct<&'static T>);

            const V: i32 = 10;
            let s = AutoGenericIndirect(Struct(&V));
            assert_eq!(format!("{s:?}"), "AutoGenericIndirect(Struct(10))");
            assert_eq!(
                format!("{s:#?}"),
                "AutoGenericIndirect(\n    Struct(\n        10,\n    ),\n)",
            );
        }
    }

    mod bound {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[test]
        fn simple() {
            #[derive(Debug)]
            struct Struct<T1, T2>(#[debug("{}.{}", _0, _1)] T1, #[debug(skip)] T2);

            let s = Struct(10, 20);
            assert_eq!(format!("{s:?}"), "Struct(10.20, ..)");
            assert_eq!(format!("{s:#?}"), "Struct(\n    10.20,\n    ..\n)");
        }

        #[test]
        fn underscored_simple() {
            #[derive(Debug)]
            struct Struct<T1, T2>(#[debug("{_0}.{_1}")] T1, #[debug(skip)] T2);

            let s = Struct(10, 20);
            assert_eq!(format!("{s:?}"), "Struct(10.20, ..)");
            assert_eq!(format!("{s:#?}"), "Struct(\n    10.20,\n    ..\n)");
        }

        #[test]
        fn redundant() {
            #[derive(Debug)]
            #[debug(bound(T1: ::core::fmt::Display, T2: ::core::fmt::Display))]
            struct Struct<T1, T2>(#[debug("{}.{}", _0, _1)] T1, #[debug(skip)] T2);

            let s = Struct(10, 20);
            assert_eq!(format!("{s:?}"), "Struct(10.20, ..)");
            assert_eq!(format!("{s:#?}"), "Struct(\n    10.20,\n    ..\n)");
        }

        #[test]
        fn underscored_redundant() {
            #[derive(Debug)]
            #[debug(bound(T1: ::core::fmt::Display, T2: ::core::fmt::Display))]
            struct Struct<T1, T2>(#[debug("{_0}.{_1}")] T1, #[debug(ignore)] T2);

            let s = Struct(10, 20);
            assert_eq!(format!("{s:?}"), "Struct(10.20, ..)");
            assert_eq!(format!("{s:#?}"), "Struct(\n    10.20,\n    ..\n)");
        }

        #[test]
        fn complex() {
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

            #[derive(Debug)]
            #[debug(bound(T1: Trait1 + Trait2, T2: Trait1 + Trait2))]
            struct Struct<T1, T2>(
                #[debug("{}_{}_{}_{}", _0.function1(), _0, _1.function2(), _1)] T1,
                T2,
            );

            let s = Struct(10, 20);
            assert_eq!(format!("{s:?}"), "Struct(WHAT_10_EVER_20, 20)");
            assert_eq!(
                format!("{s:#?}"),
                "Struct(\n    WHAT_10_EVER_20,\n    20,\n)",
            );
        }

        #[test]
        fn underscored_complex() {
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

            #[derive(Debug)]
            #[debug(bound(T1: Trait1 + Trait2, T2: Trait1 + Trait2))]
            struct Struct<T1, T2>(
                #[debug("{}_{_0}_{}_{_1}", _0.function1(), _1.function2())] T1,
                T2,
            );

            let s = Struct(10, 20);
            assert_eq!(format!("{s:?}"), "Struct(WHAT_10_EVER_20, 20)");
            assert_eq!(
                format!("{s:#?}"),
                "Struct(\n    WHAT_10_EVER_20,\n    20,\n)",
            );
        }
    }

    mod transparency {
        #[cfg(not(feature = "std"))]
        use alloc::format;

        use derive_more::Debug;

        #[derive(Debug)]
        #[debug("{0:o}", _0)]
        struct Tuple<T>(T);

        #[derive(Debug)]
        #[debug("{named:e}", named = b)]
        struct Struct<A, B> {
            a: A,
            b: B,
        }

        #[derive(Debug)]
        enum Enum<A, B, C> {
            #[debug("{_0:?}")]
            Debug(A),
            #[debug("{}", c)]
            Display { b: B, c: C },
        }

        #[test]
        fn assert() {
            assert_eq!(format!("{:03?}", Tuple(9)), "011");
            assert_eq!(format!("{:.1?}", Struct { a: 9, b: 3.15 }), "3.1e0");
            assert_eq!(format!("{:03?}", Enum::<_, u8, u8>::Debug(7)), "007");
            assert_eq!(
                format!("{:03?}", Enum::<u8, _, _>::Display { b: 7, c: 8 }),
                "008",
            );
        }

        mod omitted {
            mod on_modifiers {
                #[cfg(not(feature = "std"))]
                use alloc::format;

                use derive_more::Debug;

                #[derive(Debug)]
                enum Enum<A, B, C, D> {
                    #[debug("{_0:x?}")]
                    LowerDebug(A),
                    #[debug("{_0:X?}")]
                    UpperDebug(B),
                    #[debug("{:^}", _0)]
                    Align(C),
                    #[debug("{:+}", _0)]
                    Sign(C),
                    #[debug("{:#b}", _0)]
                    Alternate(C),
                    #[debug("{:0}", _0)]
                    ZeroPadded(C),
                    #[debug("{:07}", _0)]
                    Width(C),
                    #[debug("{:.5}", _0)]
                    Precision(D),
                }

                #[test]
                fn assert() {
                    assert_eq!(
                        format!("{:03?}", Enum::<_, u8, u8, f64>::LowerDebug(7)),
                        "7",
                    );
                    assert_eq!(
                        format!("{:03?}", Enum::<u8, _, u8, f64>::UpperDebug(8)),
                        "8",
                    );
                    assert_eq!(
                        format!("{:03?}", Enum::<u8, u8, _, f64>::Align(5)),
                        "5",
                    );
                    assert_eq!(
                        format!("{:03?}", Enum::<u8, u8, _, f64>::Sign(5)),
                        "+5",
                    );
                    assert_eq!(
                        format!("{:07?}", Enum::<u8, u8, _, f64>::Alternate(5)),
                        "0b101",
                    );
                    assert_eq!(
                        format!("{:07?}", Enum::<u8, u8, _, f64>::ZeroPadded(-5)),
                        "-5",
                    );
                    assert_eq!(
                        format!("{:03?}", Enum::<u8, u8, _, f64>::Width(5)),
                        "0000005",
                    );
                    assert_eq!(
                        format!("{:.3?}", Enum::<u8, u8, u8, _>::Precision(1.23456789)),
                        "1.23457",
                    );
                }
            }
        }
    }
}

// See: https://github.com/JelteF/derive_more/issues/301
mod complex_enum_syntax {
    #[cfg(not(feature = "std"))]
    use alloc::format;

    use derive_more::Debug;

    #[derive(Debug)]
    enum Enum {
        A = if cfg!(unix) { 2 } else { 3 },
    }

    #[test]
    fn assert() {
        assert_eq!(format!("{:?}", Enum::A), "A");
    }
}

// See: https://github.com/JelteF/derive_more/issues/363
mod type_variables {
    mod our_alloc {
        #[cfg(not(feature = "std"))]
        pub use alloc::{boxed::Box, format, vec, vec::Vec};
        #[cfg(feature = "std")]
        pub use std::{boxed::Box, format, vec, vec::Vec};
    }

    use our_alloc::{format, vec, Box, Vec};

    use derive_more::Debug;

    #[derive(Debug)]
    struct ItemStruct {
        next: Option<Box<ItemStruct>>,
    }

    #[derive(Debug)]
    struct ItemTuple(Option<Box<ItemTuple>>);

    #[derive(Debug)]
    #[debug("Item({_0:?})")]
    struct ItemTupleContainerFmt(Option<Box<ItemTupleContainerFmt>>);

    #[derive(Debug)]
    enum ItemEnum {
        Node { children: Vec<ItemEnum>, inner: i32 },
        Leaf { inner: i32 },
    }

    #[derive(Debug)]
    struct VecMeansDifferent<Vec> {
        next: our_alloc::Vec<i32>,
        real: Vec,
    }

    #[derive(Debug)]
    struct Array<T> {
        #[debug("{t}")]
        t: [T; 10],
    }

    mod parens {
        #![allow(unused_parens)] // test that type is found even in parentheses

        use derive_more::Debug;

        #[derive(Debug)]
        struct Paren<T> {
            t: (T),
        }
    }

    #[derive(Debug)]
    struct ParenthesizedGenericArgumentsInput<T> {
        t: dyn Fn(T) -> i32,
    }

    #[derive(Debug)]
    struct ParenthesizedGenericArgumentsOutput<T> {
        t: dyn Fn(i32) -> T,
    }

    #[derive(Debug)]
    struct Ptr<T> {
        t: *const T,
    }

    #[derive(Debug)]
    struct Reference<'a, T> {
        t: &'a T,
    }

    #[derive(Debug)]
    struct Slice<'a, T> {
        t: &'a [T],
    }

    #[derive(Debug)]
    struct BareFn<T> {
        t: Box<fn(T) -> T>,
    }

    #[derive(Debug)]
    struct Tuple<T> {
        t: Box<(T, T)>,
    }

    trait MyTrait<T> {}

    #[derive(Debug)]
    struct TraitObject<T> {
        t: Box<dyn MyTrait<T>>,
    }

    #[test]
    fn assert() {
        assert_eq!(
            format!(
                "{:?}",
                ItemStruct {
                    next: Some(Box::new(ItemStruct { next: None }))
                },
            ),
            "ItemStruct { next: Some(ItemStruct { next: None }) }",
        );

        assert_eq!(
            format!("{:?}", ItemTuple(Some(Box::new(ItemTuple(None))))),
            "ItemTuple(Some(ItemTuple(None)))",
        );

        assert_eq!(
            format!(
                "{:?}",
                ItemTupleContainerFmt(Some(Box::new(ItemTupleContainerFmt(None)))),
            ),
            "Item(Some(Item(None)))",
        );

        let item = ItemEnum::Node {
            children: vec![
                ItemEnum::Node {
                    children: vec![],
                    inner: 0,
                },
                ItemEnum::Leaf { inner: 1 },
            ],
            inner: 2,
        };
        assert_eq!(
            format!("{item:?}"),
            "Node { children: [Node { children: [], inner: 0 }, Leaf { inner: 1 }], inner: 2 }",
        )
    }
}
