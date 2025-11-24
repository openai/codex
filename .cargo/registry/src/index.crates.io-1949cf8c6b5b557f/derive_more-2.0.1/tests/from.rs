#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(nightly, feature(never_type))]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{
    borrow::Cow,
    string::{String, ToString},
};
#[cfg(feature = "std")]
use std::borrow::Cow;

use derive_more::From;
use static_assertions::assert_not_impl_any;

mod structs {
    use super::*;

    mod unit {
        use super::*;

        #[derive(Debug, From, PartialEq)]
        struct Unit;

        #[derive(Debug, From, PartialEq)]
        struct Tuple();

        #[derive(Debug, From, PartialEq)]
        struct Struct {}

        #[test]
        fn assert() {
            assert_eq!(Unit, ().into());
            assert_eq!(Tuple(), ().into());
            assert_eq!(Struct {}, ().into());
        }

        mod generic {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            struct Unit<const N: usize>;

            #[derive(Debug, From, PartialEq)]
            struct Tuple<const N: usize>();

            #[derive(Debug, From, PartialEq)]
            struct Struct<const N: usize> {}

            #[test]
            fn assert() {
                assert_eq!(Unit::<1>, ().into());
                assert_eq!(Tuple::<1>(), ().into());
                assert_eq!(Struct::<1> {}, ().into());
            }
        }
    }

    mod single_field {
        use super::*;

        #[derive(Debug, From, PartialEq)]
        struct Tuple(i32);

        #[derive(Debug, From, PartialEq)]
        struct Struct {
            field: i32,
        }

        #[test]
        fn assert() {
            assert_eq!(Tuple(42), 42.into());
            assert_eq!(Struct { field: 42 }, 42.into());
        }

        mod types {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            #[from(i8)]
            #[from(i16)]
            struct Tuple(i32);

            #[derive(Debug, From, PartialEq)]
            #[from(&str, Cow<'_, str>)]
            struct Struct {
                field: String,
            }

            #[test]
            fn assert() {
                assert_not_impl_any!(Tuple: From<i32>);
                assert_not_impl_any!(Struct: From<String>);

                assert_eq!(Tuple(42), 42_i8.into());
                assert_eq!(Tuple(42), 42_i16.into());
                assert_eq!(
                    Struct {
                        field: "42".to_string(),
                    },
                    "42".into(),
                );
                assert_eq!(
                    Struct {
                        field: "42".to_string(),
                    },
                    Cow::Borrowed("42").into(),
                );
            }
        }

        mod forward {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            #[from(forward)]
            struct Tuple(i32);

            #[derive(Debug, From, PartialEq)]
            #[from(forward)]
            struct Struct {
                field: String,
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple(42), 42_i8.into());
                assert_eq!(Tuple(42), 42_i16.into());
                assert_eq!(Tuple(42), 42_i32.into());
                assert_eq!(
                    Struct {
                        field: "42".to_string(),
                    },
                    "42".into(),
                );
                assert_eq!(
                    Struct {
                        field: "42".to_string(),
                    },
                    Cow::Borrowed("42").into(),
                );
            }
        }

        mod generic {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            struct Tuple<T>(T);

            #[derive(Debug, From, PartialEq)]
            struct Struct<T> {
                field: T,
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple(42), 42.into());
                assert_eq!(Struct { field: 42 }, 42.into());
            }

            mod reference {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                struct Tuple<'a, T>(&'a T);

                #[derive(Debug, From, PartialEq)]
                struct Struct<'a, T> {
                    field: &'a T,
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple(&42), (&42).into());
                    assert_eq!(Struct { field: &42 }, (&42).into());
                }
            }

            mod indirect {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                struct Tuple<T: 'static>(&'static T);

                #[derive(Debug, From, PartialEq)]
                struct Struct<T: 'static> {
                    field: &'static T,
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple(&42), (&42).into());
                    assert_eq!(Struct { field: &42 }, (&42).into());
                }
            }

            mod bounded {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                struct Tuple<T: Clone>(T);

                #[derive(Debug, From, PartialEq)]
                struct Struct<T: Clone> {
                    field: T,
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple(42), 42.into());
                    assert_eq!(Struct { field: 42 }, 42.into());
                }
            }

            mod r#const {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                struct Tuple<const N: usize, T>(T);

                #[derive(Debug, From, PartialEq)]
                struct Struct<T, const N: usize> {
                    field: T,
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::<1, _>(1), 1.into());
                    assert_eq!(Struct::<_, 1> { field: 1 }, 1.into());
                }
            }
        }
    }

    mod multi_field {
        use super::*;

        #[derive(Debug, From, PartialEq)]
        struct Tuple(i32, i16);

        #[derive(Debug, From, PartialEq)]
        struct Struct {
            field1: i32,
            field2: i16,
        }

        #[test]
        fn assert() {
            assert_eq!(Tuple(0, 1), (0, 1_i16).into());
            assert_eq!(
                Struct {
                    field1: 0,
                    field2: 1,
                },
                (0, 1_i16).into(),
            );
        }

        mod types {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            #[from((i16, i16))]
            struct Tuple(i32, i16);

            #[derive(Debug, From, PartialEq)]
            #[from((i16, i16))]
            struct Struct {
                field1: i32,
                field2: i16,
            }

            #[test]
            fn assert() {
                assert_not_impl_any!(Tuple: From<(i32, i16)>);
                assert_not_impl_any!(Struct: From<(i32, i16)>);

                assert_eq!(Tuple(0, 1), (0_i16, 1_i16).into());
                assert_eq!(
                    Struct {
                        field1: 0,
                        field2: 1
                    },
                    (0_i16, 1_i16).into(),
                );
            }
        }

        mod forward {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            #[from(forward)]
            struct Tuple(i32, i16);

            #[derive(Debug, From, PartialEq)]
            #[from(forward)]
            struct Struct {
                field1: i32,
                field2: i16,
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple(0, 1), (0_i8, 1_i8).into());
                assert_eq!(Tuple(0, 1), (0_i8, 1_i16).into());
                assert_eq!(Tuple(0, 1), (0_i16, 1_i8).into());
                assert_eq!(Tuple(0, 1), (0_i16, 1_i16).into());
                assert_eq!(Tuple(0, 1), (0_i32, 1_i8).into());
                assert_eq!(Tuple(0, 1), (0_i32, 1_i16).into());
                assert_eq!(
                    Struct {
                        field1: 0,
                        field2: 1
                    },
                    (0_i8, 1_i8).into(),
                );
                assert_eq!(
                    Struct {
                        field1: 0,
                        field2: 1
                    },
                    (0_i8, 1_i16).into(),
                );
                assert_eq!(
                    Struct {
                        field1: 0,
                        field2: 1
                    },
                    (0_i16, 1_i8).into(),
                );
                assert_eq!(
                    Struct {
                        field1: 0,
                        field2: 1
                    },
                    (0_i16, 1_i16).into(),
                );
                assert_eq!(
                    Struct {
                        field1: 0,
                        field2: 1
                    },
                    (0_i32, 1_i8).into(),
                );
                assert_eq!(
                    Struct {
                        field1: 0,
                        field2: 1
                    },
                    (0_i32, 1_i16).into(),
                );
            }
        }

        mod generic {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            struct Tuple<A, B>(A, B);

            #[derive(Debug, From, PartialEq)]
            struct Struct<A, B> {
                field1: A,
                field2: B,
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple(1, 2_i8), (1, 2_i8).into());
                assert_eq!(
                    Struct {
                        field1: 1,
                        field2: 2_i8,
                    },
                    (1, 2_i8).into(),
                );
            }

            mod reference {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                struct Tuple<'a, A, B>(&'a A, &'a B);

                #[derive(Debug, From, PartialEq)]
                struct Struct<'a, A, B> {
                    field1: &'a A,
                    field2: &'a B,
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple(&1, &2_i8), (&1, &2_i8).into());
                    assert_eq!(
                        Struct {
                            field1: &1,
                            field2: &2_i8,
                        },
                        (&1, &2_i8).into(),
                    );
                }
            }

            mod bounded {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                struct Tuple<A: Clone, B>(A, B);

                #[derive(Debug, From, PartialEq)]
                struct Struct<A: Clone, B> {
                    field1: A,
                    field2: B,
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple(1, 2_i8), (1, 2_i8).into());
                    assert_eq!(
                        Struct {
                            field1: 1,
                            field2: 2_i8,
                        },
                        (1, 2_i8).into(),
                    );
                }
            }

            mod r#const {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                struct ConstTuple<const N: usize, A, B>(A, B);

                #[derive(Debug, From, PartialEq)]
                struct ConstStruct<const N: usize, A, B> {
                    field1: A,
                    field2: B,
                }

                #[test]
                fn assert() {
                    assert_eq!(ConstTuple::<1, _, _>(1, 2_i8), (1, 2_i8).into());
                    assert_eq!(
                        ConstStruct::<1, _, _> {
                            field1: 1,
                            field2: 2_i8,
                        },
                        (1, 2_i8).into(),
                    );
                }
            }
        }

        #[cfg(nightly)]
        mod never {
            use super::*;

            #[derive(From)]
            struct Tuple(i32, !);

            #[derive(From)]
            struct Struct {
                field1: !,
                field2: i16,
            }
        }
    }
}

mod enums {
    use super::*;

    mod unit_variant {
        use super::*;

        #[derive(Debug, From, PartialEq)]
        enum Enum {
            #[from]
            Unit,
            Unnamed(),
            Named {},
        }

        #[test]
        fn assert() {
            assert_eq!(Enum::Unit, ().into());
        }

        mod generic {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Unit {
                Variant,
            }

            #[derive(Debug, From, PartialEq)]
            enum Tuple {
                Variant(),
            }

            #[derive(Debug, From, PartialEq)]
            enum Struct {
                Variant {},
            }

            assert_not_impl_any!(Unit: From<()>);
            assert_not_impl_any!(Tuple: From<()>);
            assert_not_impl_any!(Struct: From<()>);

            mod r#const {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Unit<const N: usize> {
                    Variant,
                }

                #[derive(Debug, From, PartialEq)]
                enum Tuple<const N: usize> {
                    Variant(),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<const N: usize> {
                    Variant {},
                }

                assert_not_impl_any!(Unit<0>: From<()>);
                assert_not_impl_any!(Tuple<0>: From<()>);
                assert_not_impl_any!(Struct<0>: From<()>);
            }
        }

        mod from {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Unit {
                #[from]
                Variant,
                AutomaticallySkipped,
            }

            #[derive(Debug, From, PartialEq)]
            enum Tuple {
                #[from]
                Variant(),
                AutomaticallySkipped(),
            }

            #[derive(Debug, From, PartialEq)]
            enum Struct {
                #[from]
                Variant {},
                AutomaticallySkipped {},
            }

            #[test]
            fn assert() {
                assert_eq!(Unit::Variant, ().into());
                assert_eq!(Tuple::Variant(), ().into());
                assert_eq!(Struct::Variant {}, ().into());
            }

            mod r#const {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Unit<const N: usize> {
                    #[from]
                    Variant,
                }

                #[derive(Debug, From, PartialEq)]
                enum Tuple<const N: usize> {
                    #[from]
                    Variant(),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<const N: usize> {
                    #[from]
                    Variant {},
                }

                #[test]
                fn assert() {
                    assert_eq!(Unit::<0>::Variant, ().into());
                    assert_eq!(Tuple::<0>::Variant(), ().into());
                    assert_eq!(Struct::<0>::Variant {}, ().into());
                }
            }
        }
    }

    mod single_field_variant {
        use super::*;

        #[derive(Debug, From, PartialEq)]
        enum Enum {
            Unnamed(i8),
            Named { field: i16 },
        }

        #[test]
        fn assert() {
            assert_eq!(Enum::Unnamed(1), 1_i8.into());
            assert_eq!(Enum::Named { field: 1 }, 1_i16.into());
        }

        mod generic {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Tuple<T> {
                Variant(T),
            }

            #[derive(Debug, From, PartialEq)]
            enum Struct<T> {
                Variant { field: T },
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple::Variant(1), 1.into());
                assert_eq!(Struct::Variant { field: 1 }, 1.into());
            }

            mod reference {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<'a, T> {
                    Variant(&'a T),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<'a, T> {
                    Variant { field: &'a T },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant(&1), (&1).into());
                    assert_eq!(Struct::Variant { field: &1 }, (&1).into());
                }
            }

            mod indirect {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<T: 'static> {
                    Variant(&'static T),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<T: 'static> {
                    Variant { field: &'static T },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant(&1), (&1).into());
                    assert_eq!(Struct::Variant { field: &1 }, (&1).into());
                }
            }

            mod bounded {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<T: Clone> {
                    Variant(T),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<T: Clone> {
                    Variant { field: T },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant(1), 1.into());
                    assert_eq!(Struct::Variant { field: 1 }, 1.into());
                }
            }

            mod r#const {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<T, const N: usize> {
                    Variant(T),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<const N: usize, T> {
                    Variant { field: T },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant::<_, 1>(1), 1.into());
                    assert_eq!(Struct::Variant::<1, _> { field: 1 }, 1.into());
                }
            }
        }

        mod from {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Tuple {
                #[from]
                Variant(i8),
                AutomaticallySkipped(i8),
            }

            #[derive(Debug, From, PartialEq)]
            enum Struct {
                #[from]
                Variant {
                    field: i8,
                },
                AutomaticallySkipped {
                    field: i8,
                },
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple::Variant(1), 1_i8.into());
                assert_eq!(Struct::Variant { field: 1 }, 1_i8.into());
            }
        }

        mod skip {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Enum {
                Unnamed(i8),
                #[from(skip)]
                UnnamedSkipped(i8),
                Named {
                    field: i16,
                },
                #[from(skip)]
                NamedSkipped {
                    field: i16,
                },
            }

            #[test]
            fn assert() {
                assert_eq!(Enum::Unnamed(1), 1_i8.into());
                assert_eq!(Enum::Named { field: 1 }, 1_i16.into());
            }

            mod generic {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<T> {
                    Variant(T),
                    #[from(skip)]
                    Skipped(T),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<T> {
                    Variant {
                        field: T,
                    },
                    #[from(skip)]
                    Skipped {
                        field: T,
                    },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant(1), 1.into());
                    assert_eq!(Struct::Variant { field: 1 }, 1.into());
                }

                mod reference {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<'a, T> {
                        Variant(&'a T),
                        #[from(skip)]
                        Skipped(&'a T),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<'a, T> {
                        Variant {
                            field: &'a T,
                        },
                        #[from(skip)]
                        Skipped {
                            field: &'a T,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(Tuple::Variant(&1), (&1).into());
                        assert_eq!(Struct::Variant { field: &1 }, (&1).into());
                    }
                }

                mod indirect {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<T: 'static> {
                        Variant(&'static T),
                        #[from(skip)]
                        Skipped(&'static T),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<T: 'static> {
                        Variant {
                            field: &'static T,
                        },
                        #[from(skip)]
                        Skipped {
                            field: &'static T,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(Tuple::Variant(&1), (&1).into());
                        assert_eq!(Struct::Variant { field: &1 }, (&1).into());
                    }
                }

                mod bounded {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<T: Clone> {
                        Variant(T),
                        #[from(skip)]
                        Skipped(T),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<T: Clone> {
                        Variant {
                            field: T,
                        },
                        #[from(skip)]
                        Skipped {
                            field: T,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(Tuple::Variant(1), 1.into());
                        assert_eq!(Struct::Variant { field: 1 }, 1.into());
                    }
                }

                mod r#const {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<T, const N: usize> {
                        Variant(T),
                        #[from(skip)]
                        Skipped(T),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<const N: usize, T> {
                        Variant {
                            field: T,
                        },
                        #[from(skip)]
                        Skipped {
                            field: T,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(Tuple::Variant::<_, 1>(1), 1.into());
                        assert_eq!(Struct::Variant::<1, _> { field: 1 }, 1.into());
                    }
                }

                mod concrete {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<T> {
                        Variant(i32),
                        #[from(skip)]
                        Skipped(T),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<T> {
                        Variant {
                            field: i32,
                        },
                        #[from(skip)]
                        Skipped {
                            field: T,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(Tuple::Variant::<i32>(1), 1.into());
                        assert_eq!(Struct::Variant::<i32> { field: 1 }, 1.into());
                    }

                    mod reference {
                        use super::*;

                        #[derive(Debug, From, PartialEq)]
                        enum Tuple<'a, T> {
                            Variant(&'a i32),
                            #[from(skip)]
                            Skipped(&'a T),
                        }

                        #[derive(Debug, From, PartialEq)]
                        enum Struct<'a, T> {
                            Variant {
                                field: &'a i32,
                            },
                            #[from(skip)]
                            Skipped {
                                field: &'a T,
                            },
                        }

                        #[test]
                        fn assert() {
                            assert_eq!(Tuple::Variant::<i32>(&1), (&1).into());
                            assert_eq!(
                                Struct::Variant::<i32> { field: &1 },
                                (&1).into()
                            );
                        }
                    }

                    mod indirect {
                        use super::*;

                        #[derive(Debug, From, PartialEq)]
                        enum Tuple<T: 'static> {
                            Variant(&'static i32),
                            #[from(skip)]
                            Skipped(&'static T),
                        }

                        #[derive(Debug, From, PartialEq)]
                        enum Struct<T: 'static> {
                            Variant {
                                field: &'static i32,
                            },
                            #[from(skip)]
                            Skipped {
                                field: &'static T,
                            },
                        }

                        #[test]
                        fn assert() {
                            assert_eq!(Tuple::Variant::<i32>(&1), (&1).into());
                            assert_eq!(
                                Struct::Variant::<i32> { field: &1 },
                                (&1).into()
                            );
                        }
                    }

                    mod bounded {
                        use super::*;

                        #[derive(Debug, From, PartialEq)]
                        enum Tuple<T: Clone> {
                            Variant(i32),
                            #[from(skip)]
                            Skipped(T),
                        }

                        #[derive(Debug, From, PartialEq)]
                        enum Struct<T: Clone> {
                            Variant {
                                field: i32,
                            },
                            #[from(skip)]
                            Skipped {
                                field: T,
                            },
                        }

                        #[test]
                        fn assert() {
                            assert_eq!(Tuple::Variant::<i32>(1), 1.into());
                            assert_eq!(Struct::Variant::<i32> { field: 1 }, 1.into());
                        }
                    }

                    mod r#const {
                        use super::*;

                        #[derive(Debug, From, PartialEq)]
                        enum Tuple<T, const N: usize> {
                            Variant(i32),
                            #[from(skip)]
                            Skipped(T),
                        }

                        #[derive(Debug, From, PartialEq)]
                        enum Struct<const N: usize, T> {
                            Variant {
                                field: i32,
                            },
                            #[from(skip)]
                            Skipped {
                                field: T,
                            },
                        }

                        #[test]
                        fn assert() {
                            assert_eq!(Tuple::Variant::<i32, 1>(1), 1.into());
                            assert_eq!(
                                Struct::Variant::<1, i32> { field: 1 },
                                1.into()
                            );
                        }
                    }
                }
            }
        }

        mod types {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Enum {
                #[from(i8)]
                Unnamed(i16),
                #[from(i16)]
                Named {
                    field: i32,
                },
                AutomaticallySkipped(i32),
            }

            #[test]
            fn assert() {
                assert_not_impl_any!(Enum: From<i32>);
                assert_eq!(Enum::Unnamed(1), 1_i8.into());
                assert_eq!(Enum::Named { field: 1 }, 1_i16.into());
            }
        }

        mod forward {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Unnamed {
                #[from(forward)]
                Variant(i32),
                AutomaticallyIgnored(i32),
            }

            #[derive(Debug, From, PartialEq)]
            enum Named {
                #[from(forward)]
                Variant {
                    field: i32,
                },
                AutomaticallyIgnored {
                    field: i32,
                },
            }

            #[test]
            fn assert() {
                assert_eq!(Unnamed::Variant(1), 1_i8.into());
                assert_eq!(Unnamed::Variant(1), 1_i16.into());
                assert_eq!(Unnamed::Variant(1), 1_i32.into());
                assert_eq!(Named::Variant { field: 1 }, 1_i8.into());
                assert_eq!(Named::Variant { field: 1 }, 1_i16.into());
                assert_eq!(Named::Variant { field: 1 }, 1_i32.into());
            }
        }
    }

    mod multi_field_variant {
        use super::*;

        #[derive(Debug, From, PartialEq)]
        enum Enum {
            Tuple(i8, i8),
            Struct { field1: i16, field2: i16 },
        }

        #[test]
        fn assert() {
            assert_eq!(Enum::Tuple(0, 1), (0_i8, 1_i8).into());
            assert_eq!(
                Enum::Struct {
                    field1: 0,
                    field2: 1
                },
                (0_i16, 1_i16).into(),
            );
        }

        mod generic {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Tuple<A, B> {
                Variant(A, B),
            }

            #[derive(Debug, From, PartialEq)]
            enum Struct<A, B> {
                Variant { field1: A, field2: B },
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple::Variant(1, 2_i16), (1, 2_i16).into());
                assert_eq!(
                    Struct::Variant {
                        field1: 1,
                        field2: 2_i16,
                    },
                    (1, 2_i16).into(),
                );
            }

            mod reference {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<'a, A, B> {
                    Variant(&'a A, &'a B),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<'a, A, B> {
                    Variant { field1: &'a A, field2: &'a B },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant(&1, &2_i16), (&1, &2_i16).into());
                    assert_eq!(
                        Struct::Variant {
                            field1: &1,
                            field2: &2_i16,
                        },
                        (&1, &2_i16).into(),
                    );
                }
            }

            mod indirect {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<A: 'static, B: 'static> {
                    Variant(&'static A, &'static B),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<A: 'static, B: 'static> {
                    Variant {
                        field1: &'static A,
                        field2: &'static B,
                    },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant(&1, &2_i16), (&1, &2_i16).into());
                    assert_eq!(
                        Struct::Variant {
                            field1: &1,
                            field2: &2_i16,
                        },
                        (&1, &2_i16).into(),
                    );
                }
            }

            mod bounded {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<A: Clone, B> {
                    Variant(A, B),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<A, B: Clone> {
                    Variant { field1: A, field2: B },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant(1, 2_i16), (1, 2_i16).into());
                    assert_eq!(
                        Struct::Variant {
                            field1: 1,
                            field2: 2_i16,
                        },
                        (1, 2_i16).into(),
                    );
                }
            }

            mod r#const {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<const N: usize, A, B> {
                    Variant(A, B),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<A, const N: usize, B> {
                    Variant { field1: A, field2: B },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant::<0, _, _>(1, 2_i16), (1, 2_i16).into());
                    assert_eq!(
                        Struct::<_, 1, _>::Variant {
                            field1: 1,
                            field2: 2_i16,
                        },
                        (1, 2_i16).into(),
                    );
                }
            }
        }

        mod from {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Tuple {
                #[from]
                Variant(i8, i16),
                AutomaticallySkipped(i8, i16),
            }

            #[derive(Debug, From, PartialEq)]
            enum Struct {
                #[from]
                Variant {
                    field1: i8,
                    field2: i16,
                },
                AutomaticallySkipped {
                    field1: i8,
                    field2: i16,
                },
            }

            #[test]
            fn assert() {
                assert_eq!(Tuple::Variant(1, 2), (1_i8, 2_i16).into());
                assert_eq!(
                    Struct::Variant {
                        field1: 1,
                        field2: 2,
                    },
                    (1_i8, 2_i16).into(),
                );
            }
        }

        mod skip {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Enum {
                Tuple(i8, i8),
                #[from(skip)]
                TupleSkipped(i8, i8),
                Struct {
                    field1: i16,
                    field2: i16,
                },
                #[from(skip)]
                StructSkipped {
                    field1: i16,
                    field2: i16,
                },
            }

            #[test]
            fn assert() {
                assert_eq!(Enum::Tuple(0, 1), (0_i8, 1_i8).into());
                assert_eq!(
                    Enum::Struct {
                        field1: 0,
                        field2: 1
                    },
                    (0_i16, 1_i16).into(),
                );
            }

            mod generic {
                use super::*;

                #[derive(Debug, From, PartialEq)]
                enum Tuple<A, B> {
                    Variant(A, B),
                    #[from(skip)]
                    Skipped(A, B),
                }

                #[derive(Debug, From, PartialEq)]
                enum Struct<A, B> {
                    Variant {
                        field1: A,
                        field2: B,
                    },
                    #[from(skip)]
                    Skipped {
                        field1: A,
                        field2: B,
                    },
                }

                #[test]
                fn assert() {
                    assert_eq!(Tuple::Variant(1, 2_i16), (1, 2_i16).into());
                    assert_eq!(
                        Struct::Variant {
                            field1: 1,
                            field2: 2_i16,
                        },
                        (1, 2_i16).into(),
                    );
                }

                mod reference {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<'a, A, B> {
                        Variant(&'a A, &'a B),
                        #[from(skip)]
                        Skipped(&'a A, &'a B),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<'a, A, B> {
                        Variant {
                            field1: &'a A,
                            field2: &'a B,
                        },
                        #[from(skip)]
                        Skipped {
                            field1: &'a A,
                            field2: &'a B,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(Tuple::Variant(&1, &2_i16), (&1, &2_i16).into());
                        assert_eq!(
                            Struct::Variant {
                                field1: &1,
                                field2: &2_i16,
                            },
                            (&1, &2_i16).into(),
                        );
                    }
                }

                mod indirect {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<A: 'static, B: 'static> {
                        Variant(&'static A, &'static B),
                        #[from(skip)]
                        Skipped(&'static A, &'static B),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<A: 'static, B: 'static> {
                        Variant {
                            field1: &'static A,
                            field2: &'static B,
                        },
                        #[from(skip)]
                        Skipped {
                            field1: &'static A,
                            field2: &'static B,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(Tuple::Variant(&1, &2_i16), (&1, &2_i16).into());
                        assert_eq!(
                            Struct::Variant {
                                field1: &1,
                                field2: &2_i16,
                            },
                            (&1, &2_i16).into(),
                        );
                    }
                }

                mod bounded {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<A: Clone, B> {
                        Variant(A, B),
                        #[from(skip)]
                        Skipped(A, B),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<A, B: Clone> {
                        Variant {
                            field1: A,
                            field2: B,
                        },
                        #[from(skip)]
                        Skipped {
                            field1: A,
                            field2: B,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(Tuple::Variant(1, 2_i16), (1, 2_i16).into());
                        assert_eq!(
                            Struct::Variant {
                                field1: 1,
                                field2: 2_i16,
                            },
                            (1, 2_i16).into(),
                        );
                    }
                }

                mod r#const {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<const N: usize, A, B> {
                        Variant(A, B),
                        #[from(skip)]
                        Skipped(A, B),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<A, const N: usize, B> {
                        Variant {
                            field1: A,
                            field2: B,
                        },
                        #[from(skip)]
                        Skipped {
                            field1: A,
                            field2: B,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(
                            Tuple::Variant::<0, _, _>(1, 2_i16),
                            (1, 2_i16).into()
                        );
                        assert_eq!(
                            Struct::<_, 1, _>::Variant {
                                field1: 1,
                                field2: 2_i16,
                            },
                            (1, 2_i16).into(),
                        );
                    }
                }

                mod concrete {
                    use super::*;

                    #[derive(Debug, From, PartialEq)]
                    enum Tuple<A, B> {
                        Variant(i32, i16),
                        #[from(skip)]
                        Skipped(A, B),
                    }

                    #[derive(Debug, From, PartialEq)]
                    enum Struct<A, B> {
                        Variant {
                            field1: i32,
                            field2: i16,
                        },
                        #[from(skip)]
                        Skipped {
                            field1: A,
                            field2: B,
                        },
                    }

                    #[test]
                    fn assert() {
                        assert_eq!(
                            Tuple::Variant::<i32, i16>(1, 2_i16),
                            (1, 2_i16).into(),
                        );
                        assert_eq!(
                            Struct::Variant::<i32, i16> {
                                field1: 1,
                                field2: 2_i16,
                            },
                            (1, 2_i16).into(),
                        );
                    }

                    mod reference {
                        use super::*;

                        #[derive(Debug, From, PartialEq)]
                        enum Tuple<'a, A, B> {
                            Variant(&'a i32, &'a i16),
                            #[from(skip)]
                            Skipped(&'a A, &'a B),
                        }

                        #[derive(Debug, From, PartialEq)]
                        enum Struct<'a, A, B> {
                            Variant {
                                field1: &'a i32,
                                field2: &'a i16,
                            },
                            #[from(skip)]
                            Skipped {
                                field1: &'a A,
                                field2: &'a B,
                            },
                        }

                        #[test]
                        fn assert() {
                            assert_eq!(
                                Tuple::Variant::<i32, i16>(&1, &2_i16),
                                (&1, &2_i16).into(),
                            );
                            assert_eq!(
                                Struct::Variant::<i32, i16> {
                                    field1: &1,
                                    field2: &2_i16,
                                },
                                (&1, &2_i16).into(),
                            );
                        }
                    }

                    mod indirect {
                        use super::*;

                        #[derive(Debug, From, PartialEq)]
                        enum Tuple<A: 'static, B: 'static> {
                            Variant(&'static i32, &'static i16),
                            #[from(skip)]
                            Skipped(&'static A, &'static B),
                        }

                        #[derive(Debug, From, PartialEq)]
                        enum Struct<A: 'static, B: 'static> {
                            Variant {
                                field1: &'static i32,
                                field2: &'static i16,
                            },
                            #[from(skip)]
                            Skipped {
                                field1: &'static A,
                                field2: &'static B,
                            },
                        }

                        #[test]
                        fn assert() {
                            assert_eq!(
                                Tuple::Variant::<i32, i16>(&1, &2_i16),
                                (&1, &2_i16).into(),
                            );
                            assert_eq!(
                                Struct::Variant::<i32, i16> {
                                    field1: &1,
                                    field2: &2_i16,
                                },
                                (&1, &2_i16).into(),
                            );
                        }
                    }

                    mod bounded {
                        use super::*;

                        #[derive(Debug, From, PartialEq)]
                        enum Tuple<A: Clone, B> {
                            Variant(i32, i16),
                            #[from(skip)]
                            Skipped(A, B),
                        }

                        #[derive(Debug, From, PartialEq)]
                        enum Struct<A, B: Clone> {
                            Variant {
                                field1: i32,
                                field2: i16,
                            },
                            #[from(skip)]
                            Skipped {
                                field1: A,
                                field2: B,
                            },
                        }

                        #[test]
                        fn assert() {
                            assert_eq!(
                                Tuple::Variant::<i32, i16>(1, 2_i16),
                                (1, 2_i16).into(),
                            );
                            assert_eq!(
                                Struct::Variant::<i32, i16> {
                                    field1: 1,
                                    field2: 2_i16,
                                },
                                (1, 2_i16).into(),
                            );
                        }
                    }

                    mod r#const {
                        use super::*;

                        #[derive(Debug, From, PartialEq)]
                        enum Tuple<const N: usize, A, B> {
                            Variant(i32, i16),
                            #[from(skip)]
                            Skipped(A, B),
                        }

                        #[derive(Debug, From, PartialEq)]
                        enum Struct<A, const N: usize, B> {
                            Variant {
                                field1: i32,
                                field2: i16,
                            },
                            #[from(skip)]
                            Skipped {
                                field1: A,
                                field2: B,
                            },
                        }

                        #[test]
                        fn assert() {
                            assert_eq!(
                                Tuple::Variant::<0, i32, i16>(1, 2_i16),
                                (1, 2_i16).into()
                            );
                            assert_eq!(
                                Struct::<i32, 1, i16>::Variant {
                                    field1: 1,
                                    field2: 2_i16,
                                },
                                (1, 2_i16).into(),
                            );
                        }
                    }
                }
            }
        }

        mod types {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Enum {
                #[from((i8, i8))]
                Tuple(i16, i16),
                #[from((i16, i16))]
                Struct {
                    field1: i32,
                    field2: i32,
                },
                AutomaticallySkipped {
                    field1: i32,
                    field2: i32,
                },
            }

            #[test]
            fn assert() {
                assert_not_impl_any!(Enum: From<(i32, i32)>);
                assert_eq!(Enum::Tuple(0, 1), (0_i8, 1_i8).into());
                assert_eq!(
                    Enum::Struct {
                        field1: 0,
                        field2: 1
                    },
                    (0_i16, 1_i16).into(),
                );
            }
        }

        mod forward {
            use super::*;

            #[derive(Debug, From, PartialEq)]
            enum Unnamed {
                #[from(forward)]
                Variant(i16, i16),
                AutomaticallyIgnored(i16, i16),
            }

            #[derive(Debug, From, PartialEq)]
            enum Named {
                #[from(forward)]
                Variant {
                    field1: i16,
                    field2: i16,
                },
                AutomaticallyIgnored {
                    field1: i16,
                    field2: i16,
                },
            }

            #[test]
            fn assert() {
                assert_eq!(Unnamed::Variant(0, 1), (0_i8, 1_i8).into());
                assert_eq!(Unnamed::Variant(0, 1), (0_i8, 1_i16).into());
                assert_eq!(Unnamed::Variant(0, 1), (0_i16, 1_i8).into());
                assert_eq!(Unnamed::Variant(0, 1), (0_i16, 1_i16).into());
                assert_eq!(
                    Named::Variant {
                        field1: 0,
                        field2: 1
                    },
                    (0_i8, 1_i8).into(),
                );
                assert_eq!(
                    Named::Variant {
                        field1: 0,
                        field2: 1
                    },
                    (0_i8, 1_i16).into(),
                );
                assert_eq!(
                    Named::Variant {
                        field1: 0,
                        field2: 1
                    },
                    (0_i16, 1_i8).into(),
                );
                assert_eq!(
                    Named::Variant {
                        field1: 0,
                        field2: 1
                    },
                    (0_i16, 1_i16).into(),
                );
            }
        }

        #[cfg(nightly)]
        mod never {
            use super::*;

            #[derive(From)]
            enum Enum {
                Tuple(i8, !),
                Struct { field1: !, field2: i16 },
            }
        }
    }
}
