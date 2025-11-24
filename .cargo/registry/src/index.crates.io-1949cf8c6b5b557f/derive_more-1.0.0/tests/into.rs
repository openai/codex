#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{
    borrow::Cow,
    borrow::ToOwned,
    boxed::Box,
    string::{String, ToString},
};
use core::mem;
#[cfg(feature = "std")]
use std::borrow::Cow;

use derive_more::Into;
use static_assertions::assert_not_impl_any;

/// Nasty [`mem::transmute()`] that works in generic contexts
/// by [`mem::forget`]ing stuff.
///
/// It's OK for tests!
unsafe fn transmute<From, To>(from: From) -> To {
    let to = unsafe { mem::transmute_copy(&from) };
    mem::forget(from);
    to
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
struct Wrapped<T>(T);

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
struct Transmuted<T>(T);

impl<T> From<Wrapped<T>> for Transmuted<T> {
    fn from(from: Wrapped<T>) -> Self {
        // SAFETY: repr(transparent)
        unsafe { transmute(from) }
    }
}

impl<T> From<&Wrapped<T>> for &Transmuted<T> {
    fn from(from: &Wrapped<T>) -> Self {
        // SAFETY: repr(transparent)
        unsafe { transmute(from) }
    }
}

impl<T> From<&mut Wrapped<T>> for &mut Transmuted<T> {
    fn from(from: &mut Wrapped<T>) -> Self {
        // SAFETY: repr(transparent)
        unsafe { transmute(from) }
    }
}

mod unit {
    #![allow(clippy::unit_cmp)] // because of type inference in assertions

    use super::*;

    #[derive(Debug, Into, PartialEq)]
    struct Unit;

    #[derive(Debug, Into, PartialEq)]
    struct Tuple();

    #[derive(Debug, Into, PartialEq)]
    struct Struct {}

    #[test]
    fn assert() {
        assert_eq!((), Unit.into());
        assert_eq!((), Tuple().into());
        assert_eq!((), Struct {}.into());
    }

    mod generic {
        use super::*;

        #[derive(Debug, Into, PartialEq)]
        struct Unit<const N: usize>;

        #[derive(Debug, Into, PartialEq)]
        struct Tuple<const N: usize>();

        #[derive(Debug, Into, PartialEq)]
        struct Struct<const N: usize> {}

        #[test]
        fn assert() {
            assert_eq!((), Unit::<1>.into());
            assert_eq!((), Tuple::<1>().into());
            assert_eq!((), Struct::<1> {}.into());
        }
    }
}

mod single_field {
    use super::*;

    #[derive(Debug, Into, PartialEq)]
    struct Tuple(i32);

    #[derive(Debug, Into, PartialEq)]
    struct Struct {
        field: i32,
    }

    #[test]
    fn assert() {
        assert_eq!(42, Tuple(42).into());
        assert_eq!(42, Struct { field: 42 }.into());
    }

    mod skip {
        use super::*;

        #[derive(Debug, Into, PartialEq)]
        struct Tuple(#[into(skip)] i32);

        #[derive(Debug, Into, PartialEq)]
        struct Struct {
            #[into(skip)]
            field: i32,
        }

        #[test]
        fn assert() {
            #![allow(clippy::unit_cmp)] // because of type inference in assertions

            assert_eq!((), Tuple(42).into());
            assert_eq!((), Struct { field: 42 }.into());
        }
    }

    mod types {
        use super::*;

        #[derive(Debug, Into, PartialEq)]
        #[into(i64)]
        #[into(i128)]
        struct Tuple(i32);

        #[derive(Debug, Into, PartialEq)]
        #[into(Box<str>, Cow<'_, str>)]
        struct Struct {
            field: String,
        }

        #[test]
        fn assert() {
            assert_not_impl_any!(Tuple: Into<i32>);
            assert_not_impl_any!(Struct: Into<String>);

            assert_eq!(42_i64, Tuple(42).into());
            assert_eq!(42_i128, Tuple(42).into());
            assert_eq!(
                Box::<str>::from("42".to_owned()),
                Struct {
                    field: "42".to_string(),
                }
                .into(),
            );
            assert_eq!(
                Cow::Borrowed("42"),
                Cow::<str>::from(Struct {
                    field: "42".to_string(),
                }),
            );
        }

        mod ref_ {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into(ref)]
            struct Unnamed(i32);

            #[derive(Debug, Into, PartialEq)]
            #[into(ref)]
            struct Named {
                field: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(&42, <&i32>::from(&Unnamed(42)));
                assert_eq!(&42, <&i32>::from(&Named { field: 42 }));
            }

            mod types {
                use super::*;

                #[derive(Debug, Into, PartialEq)]
                #[into(ref(i32, Unnamed))]
                struct Tuple(Unnamed);

                #[derive(Debug, Into, PartialEq)]
                #[into(ref(i32, Named))]
                struct Struct {
                    field: Named,
                }

                #[test]
                fn assert() {
                    assert_eq!(&42, <&i32>::from(&Tuple(Unnamed(42))));
                    assert_eq!(&Unnamed(42), <&Unnamed>::from(&Tuple(Unnamed(42))));
                    assert_eq!(
                        &42,
                        <&i32>::from(&Struct {
                            field: Named { field: 42 },
                        }),
                    );
                    assert_eq!(
                        &Named { field: 42 },
                        <&Named>::from(&Struct {
                            field: Named { field: 42 },
                        }),
                    );
                }
            }
        }

        mod ref_mut {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into(ref_mut)]
            struct Unnamed(i32);

            #[derive(Debug, Into, PartialEq)]
            #[into(ref_mut)]
            struct Named {
                field: i32,
            }

            #[test]
            fn assert() {
                assert_eq!(&mut 42, <&mut i32>::from(&mut Unnamed(42)));
                assert_eq!(&mut 42, <&mut i32>::from(&mut Named { field: 42 }));
            }

            mod types {
                use super::*;

                #[derive(Debug, Into, PartialEq)]
                #[into(ref_mut(i32, Unnamed))]
                struct Tuple(Unnamed);

                #[derive(Debug, Into, PartialEq)]
                #[into(ref_mut(i32, Named))]
                struct Struct {
                    field: Named,
                }

                #[test]
                fn assert() {
                    assert_eq!(&mut 42, <&mut i32>::from(&mut Tuple(Unnamed(42))));
                    assert_eq!(
                        &mut Unnamed(42),
                        <&mut Unnamed>::from(&mut Tuple(Unnamed(42))),
                    );
                    assert_eq!(
                        &mut 42,
                        <&mut i32>::from(&mut Struct {
                            field: Named { field: 42 },
                        }),
                    );
                    assert_eq!(
                        &mut Named { field: 42 },
                        <&mut Named>::from(&mut Struct {
                            field: Named { field: 42 },
                        }),
                    );
                }
            }
        }
    }

    mod generic {
        use super::*;

        #[derive(Debug, Into, PartialEq)]
        struct Tuple<T>(Wrapped<T>);

        #[derive(Debug, Into, PartialEq)]
        struct Struct<T> {
            field: Wrapped<T>,
        }

        #[test]
        fn assert() {
            assert_eq!(Wrapped(42), Tuple(Wrapped(42)).into());
            assert_eq!(Wrapped(42), Struct { field: Wrapped(42) }.into());
        }

        mod skip {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            struct Tuple<T>(#[into(skip)] Wrapped<T>);

            #[derive(Debug, Into, PartialEq)]
            struct Struct<T> {
                #[into(skip)]
                field: Wrapped<T>,
            }

            #[test]
            fn assert() {
                #![allow(clippy::unit_cmp)] // because of type inference in assertions

                assert_eq!((), Tuple(Wrapped(42)).into());
                assert_eq!((), Struct { field: Wrapped(42) }.into());
            }
        }

        mod types {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into(Transmuted<T>)]
            struct Tuple<T>(Wrapped<T>);

            #[derive(Debug, Into, PartialEq)]
            #[into(Transmuted<T>)]
            struct Struct<T> {
                field: Wrapped<T>,
            }

            #[test]
            fn assert() {
                assert_eq!(Transmuted(42), Tuple(Wrapped(42)).into());
                assert_eq!(Transmuted(42), Struct { field: Wrapped(42) }.into());
            }
        }

        mod ref_ {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into(ref)]
            struct Tuple<T>(Wrapped<T>);

            #[derive(Debug, Into, PartialEq)]
            #[into(ref)]
            struct Struct<T> {
                field: Wrapped<T>,
            }

            #[test]
            fn assert() {
                assert_eq!(&Wrapped(42), <&Wrapped<_>>::from(&Tuple(Wrapped(42))));
                assert_eq!(
                    &Wrapped(42),
                    <&Wrapped<_>>::from(&Struct { field: Wrapped(42) })
                );
            }

            mod types {
                use super::*;

                #[derive(Debug, Into, PartialEq)]
                #[into(ref(Transmuted<T>))]
                struct Tuple<T>(Wrapped<T>);

                #[derive(Debug, Into, PartialEq)]
                #[into(ref(Transmuted<T>))]
                struct Struct<T> {
                    field: Wrapped<T>,
                }

                #[test]
                fn assert() {
                    assert_eq!(
                        &Transmuted(42),
                        <&Transmuted<_>>::from(&Tuple(Wrapped(42))),
                    );
                    assert_eq!(
                        &Transmuted(42),
                        <&Transmuted<_>>::from(&Struct { field: Wrapped(42) }),
                    );
                }
            }
        }

        mod ref_mut {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into(ref_mut)]
            struct Tuple<T>(Wrapped<T>);

            #[derive(Debug, Into, PartialEq)]
            #[into(ref_mut)]
            struct Struct<T> {
                field: Wrapped<T>,
            }

            #[test]
            fn assert() {
                assert_eq!(
                    &mut Wrapped(42),
                    <&mut Wrapped<_>>::from(&mut Tuple(Wrapped(42)))
                );
                assert_eq!(
                    &mut Wrapped(42),
                    <&mut Wrapped<_>>::from(&mut Struct { field: Wrapped(42) }),
                );
            }

            mod types {
                use super::*;

                #[derive(Debug, Into, PartialEq)]
                #[into(ref_mut(Transmuted<T>))]
                struct Tuple<T>(Wrapped<T>);

                #[derive(Debug, Into, PartialEq)]
                #[into(ref_mut(Transmuted<T>))]
                struct Struct<T> {
                    field: Wrapped<T>,
                }

                #[test]
                fn assert() {
                    assert_eq!(
                        &mut Transmuted(42),
                        <&mut Transmuted<_>>::from(&mut Tuple(Wrapped(42))),
                    );
                    assert_eq!(
                        &mut Transmuted(42),
                        <&mut Transmuted<_>>::from(&mut Struct { field: Wrapped(42) }),
                    );
                }
            }
        }

        mod indirect {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            struct Tuple<T: 'static>(&'static Wrapped<T>);

            #[derive(Debug, Into, PartialEq)]
            struct Struct<T: 'static> {
                field: &'static Wrapped<T>,
            }

            #[test]
            fn assert() {
                assert_eq!(&Wrapped(42), <&Wrapped<_>>::from(Tuple(&Wrapped(42))));
                assert_eq!(
                    &Wrapped(42),
                    <&Wrapped<_>>::from(Struct {
                        field: &Wrapped(42),
                    }),
                );
            }
        }

        mod bounded {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            struct Tuple<T: Clone>(Wrapped<T>);

            #[derive(Debug, Into, PartialEq)]
            struct Struct<T: Clone> {
                field: Wrapped<T>,
            }

            #[test]
            fn assert() {
                assert_eq!(Wrapped(42), Tuple(Wrapped(42)).into());
                assert_eq!(Wrapped(42), Struct { field: Wrapped(42) }.into());
            }
        }

        mod r#const {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            struct Tuple<const N: usize, T>(Wrapped<T>);

            #[derive(Debug, Into, PartialEq)]
            struct Struct<T, const N: usize> {
                field: Wrapped<T>,
            }

            #[test]
            fn assert() {
                assert_eq!(Wrapped(1), Tuple::<1, _>(Wrapped(1)).into());
                assert_eq!(Wrapped(1), Struct::<_, 1> { field: Wrapped(1) }.into());
            }
        }
    }
}

mod multi_field {
    use super::*;

    #[derive(Debug, Into, PartialEq)]
    struct Tuple(i32, i64);

    #[derive(Debug, Into, PartialEq)]
    struct Struct {
        field1: i32,
        field2: i64,
    }

    #[test]
    fn assert() {
        assert_eq!((1, 2_i64), Tuple(1, 2_i64).into());
        assert_eq!(
            (1, 2_i64),
            Struct {
                field1: 1,
                field2: 2_i64,
            }
            .into(),
        );
    }

    mod skip {
        use super::*;

        #[derive(Debug, Into, PartialEq)]
        struct Tuple(i32, #[into(skip)] i64);

        #[derive(Debug, Into, PartialEq)]
        struct Struct {
            #[into(skip)]
            field1: i32,
            field2: i64,
        }

        #[test]
        fn assert() {
            assert_eq!(1, Tuple(1, 2_i64).into());
            assert_eq!(
                2_i64,
                Struct {
                    field1: 1,
                    field2: 2_i64,
                }
                .into(),
            );
        }
    }

    mod types {
        use super::*;

        #[derive(Debug, Into, PartialEq)]
        #[into((i32, i64))]
        #[into((i64, i128))]
        struct Tuple(i16, i32);

        #[derive(Debug, Into, PartialEq)]
        #[into((Box<str>, i32), (Cow<'_, str>, i64))]
        struct Struct {
            field1: String,
            field2: i32,
        }

        #[test]
        fn assert() {
            assert_not_impl_any!(Tuple: Into<(i16, i32)>);
            assert_not_impl_any!(Struct: Into<(String, i32)>);

            assert_eq!((1, 2_i64), Tuple(1_i16, 2).into());
            assert_eq!((1_i64, 2_i128), Tuple(1_i16, 2).into());
            assert_eq!(
                (Box::<str>::from("42".to_owned()), 1),
                Struct {
                    field1: "42".to_string(),
                    field2: 1,
                }
                .into(),
            );
            assert_eq!(
                (Cow::Borrowed("42"), 1_i64),
                Struct {
                    field1: "42".to_string(),
                    field2: 1,
                }
                .into(),
            );
        }

        mod ref_ {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into(ref)]
            struct Unnamed(i32, i64);

            #[derive(Debug, Into, PartialEq)]
            #[into(ref)]
            struct Named {
                field1: i32,
                field2: i64,
            }

            #[test]
            fn assert() {
                assert_eq!((&1, &2_i64), (&Unnamed(1, 2_i64)).into());
                assert_eq!(
                    (&1, &2_i64),
                    (&Named {
                        field1: 1,
                        field2: 2_i64,
                    })
                        .into(),
                );
            }

            mod types {
                use super::*;

                #[derive(Debug, Into, PartialEq)]
                #[into(ref(
                    (Transmuted<i32>, Transmuted<i64>),
                    (Transmuted<i32>, Wrapped<i64>)),
                )]
                struct Tuple(Wrapped<i32>, Wrapped<i64>);

                #[derive(Debug, Into, PartialEq)]
                #[into(ref(
                    (Transmuted<i32>, Transmuted<i64>),
                    (Transmuted<i32>, Wrapped<i64>)),
                )]
                struct Struct {
                    field1: Wrapped<i32>,
                    field2: Wrapped<i64>,
                }

                #[test]
                fn assert() {
                    assert_eq!(
                        (&Transmuted(1), &Transmuted(2_i64)),
                        (&Tuple(Wrapped(1), Wrapped(2_i64))).into(),
                    );
                    assert_eq!(
                        (&Transmuted(1), &Wrapped(2_i64)),
                        (&Tuple(Wrapped(1), Wrapped(2_i64))).into(),
                    );
                    assert_eq!(
                        (&Transmuted(1), &Transmuted(2_i64)),
                        (&Struct {
                            field1: Wrapped(1),
                            field2: Wrapped(2_i64),
                        })
                            .into(),
                    );
                    assert_eq!(
                        (&Transmuted(1), &Wrapped(2_i64)),
                        (&Struct {
                            field1: Wrapped(1),
                            field2: Wrapped(2_i64),
                        })
                            .into(),
                    );
                }
            }
        }

        mod ref_mut {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into(ref_mut)]
            struct Unnamed(i32, i64);

            #[derive(Debug, Into, PartialEq)]
            #[into(ref_mut)]
            struct Named {
                field1: i32,
                field2: i64,
            }

            #[test]
            fn assert() {
                assert_eq!((&mut 1, &mut 2_i64), (&mut Unnamed(1, 2_i64)).into());
                assert_eq!(
                    (&mut 1, &mut 2_i64),
                    (&mut Named {
                        field1: 1,
                        field2: 2_i64,
                    })
                        .into(),
                );
            }

            mod types {
                use super::*;

                #[derive(Debug, Into, PartialEq)]
                #[into(ref_mut(
                    (Transmuted<i32>, Transmuted<i64>),
                    (Transmuted<i32>, Wrapped<i64>)),
                )]
                struct Tuple(Wrapped<i32>, Wrapped<i64>);

                #[derive(Debug, Into, PartialEq)]
                #[into(ref_mut(
                    (Transmuted<i32>, Transmuted<i64>),
                    (Transmuted<i32>, Wrapped<i64>)),
                )]
                struct Struct {
                    field1: Wrapped<i32>,
                    field2: Wrapped<i64>,
                }

                #[test]
                fn assert() {
                    assert_eq!(
                        (&mut Transmuted(1), &mut Transmuted(2_i64)),
                        (&mut Tuple(Wrapped(1), Wrapped(2_i64))).into(),
                    );
                    assert_eq!(
                        (&mut Transmuted(1), &mut Wrapped(2_i64)),
                        (&mut Tuple(Wrapped(1), Wrapped(2_i64))).into(),
                    );
                    assert_eq!(
                        (&mut Transmuted(1), &mut Transmuted(2_i64)),
                        (&mut Struct {
                            field1: Wrapped(1),
                            field2: Wrapped(2_i64),
                        })
                            .into(),
                    );
                    assert_eq!(
                        (&mut Transmuted(1), &mut Wrapped(2_i64)),
                        (&mut Struct {
                            field1: Wrapped(1),
                            field2: Wrapped(2_i64),
                        })
                            .into(),
                    );
                }
            }
        }
    }

    mod generic {
        use super::*;

        #[derive(Debug, Into, PartialEq)]
        struct Tuple<A, B>(Wrapped<A>, Wrapped<B>);

        #[derive(Debug, Into, PartialEq)]
        struct Struct<A, B> {
            field1: Wrapped<A>,
            field2: Wrapped<B>,
        }

        #[test]
        fn assert() {
            assert_eq!(
                (Wrapped(1), Wrapped(2)),
                Tuple(Wrapped(1), Wrapped(2)).into(),
            );
            assert_eq!(
                (Wrapped(1), Wrapped(2)),
                Struct {
                    field1: Wrapped(1),
                    field2: Wrapped(2),
                }
                .into(),
            );
        }

        mod skip {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            struct Tuple<A, B>(Wrapped<A>, #[into(skip)] Wrapped<B>);

            #[derive(Debug, Into, PartialEq)]
            struct Struct<A, B> {
                #[into(skip)]
                field1: Wrapped<A>,
                field2: Wrapped<B>,
            }

            #[test]
            fn assert() {
                assert_eq!(Wrapped(1), Tuple(Wrapped(1), Wrapped(2)).into());
                assert_eq!(
                    Wrapped(2),
                    Struct {
                        field1: Wrapped(1),
                        field2: Wrapped(2),
                    }
                    .into(),
                );
            }
        }

        mod types {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into((Transmuted<A>, Transmuted<B>))]
            struct Tuple<A, B>(Wrapped<A>, Wrapped<B>);

            #[derive(Debug, Into, PartialEq)]
            #[into((Transmuted<A>, Transmuted<B>))]
            struct Struct<A, B> {
                field1: Wrapped<A>,
                field2: Wrapped<B>,
            }

            #[test]
            fn assert() {
                assert_eq!(
                    (Transmuted(1), Transmuted(2)),
                    Tuple(Wrapped(1), Wrapped(2)).into(),
                );
                assert_eq!(
                    (Transmuted(1), Transmuted(2)),
                    Struct {
                        field1: Wrapped(1),
                        field2: Wrapped(2),
                    }
                    .into(),
                );
            }
        }

        mod ref_ {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into(ref)]
            struct Tuple<A, B>(Wrapped<A>, Wrapped<B>);

            #[derive(Debug, Into, PartialEq)]
            #[into(ref)]
            struct Struct<A, B> {
                field1: Wrapped<A>,
                field2: Wrapped<B>,
            }

            #[test]
            fn assert() {
                assert_eq!(
                    (&Wrapped(1), &Wrapped(2)),
                    (&Tuple(Wrapped(1), Wrapped(2))).into(),
                );
                assert_eq!(
                    (&Wrapped(1), &Wrapped(2)),
                    (&Struct {
                        field1: Wrapped(1),
                        field2: Wrapped(2),
                    })
                        .into(),
                );
            }

            mod types {
                use super::*;

                #[derive(Debug, Into, PartialEq)]
                #[into(ref((Transmuted<A>, Transmuted<B>)))]
                struct Tuple<A, B>(Wrapped<A>, Wrapped<B>);

                #[derive(Debug, Into, PartialEq)]
                #[into(ref((Transmuted<A>, Transmuted<B>)))]
                struct Struct<A, B> {
                    field1: Wrapped<A>,
                    field2: Wrapped<B>,
                }

                #[test]
                fn assert() {
                    assert_eq!(
                        (&Transmuted(1), &Transmuted(2)),
                        (&Tuple(Wrapped(1), Wrapped(2))).into(),
                    );
                    assert_eq!(
                        (&Transmuted(1), &Transmuted(2)),
                        (&Struct {
                            field1: Wrapped(1),
                            field2: Wrapped(2),
                        })
                            .into(),
                    );
                }
            }
        }

        mod ref_mut {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            #[into(ref_mut)]
            struct Tuple<A, B>(Wrapped<A>, Wrapped<B>);

            #[derive(Debug, Into, PartialEq)]
            #[into(ref_mut)]
            struct Struct<A, B> {
                field1: Wrapped<A>,
                field2: Wrapped<B>,
            }

            #[test]
            fn assert() {
                assert_eq!(
                    (&mut Wrapped(1), &mut Wrapped(2)),
                    (&mut Tuple(Wrapped(1), Wrapped(2))).into(),
                );
                assert_eq!(
                    (&mut Wrapped(1), &mut Wrapped(2)),
                    (&mut Struct {
                        field1: Wrapped(1),
                        field2: Wrapped(2),
                    })
                        .into(),
                );
            }

            mod types {
                use super::*;

                #[derive(Debug, Into, PartialEq)]
                #[into(ref_mut((Transmuted<A>, Transmuted<B>)))]
                struct Tuple<A, B>(Wrapped<A>, Wrapped<B>);

                #[derive(Debug, Into, PartialEq)]
                #[into(ref_mut((Transmuted<A>, Transmuted<B>)))]
                struct Struct<A, B> {
                    field1: Wrapped<A>,
                    field2: Wrapped<B>,
                }

                #[test]
                fn assert() {
                    assert_eq!(
                        (&mut Transmuted(1), &mut Transmuted(2)),
                        (&mut Tuple(Wrapped(1), Wrapped(2))).into(),
                    );
                    assert_eq!(
                        (&mut Transmuted(1), &mut Transmuted(2)),
                        (&mut Struct {
                            field1: Wrapped(1),
                            field2: Wrapped(2),
                        })
                            .into(),
                    );
                }
            }
        }

        mod indirect {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            struct Tuple<A: 'static, B: 'static>(
                &'static Wrapped<A>,
                &'static Wrapped<B>,
            );

            #[derive(Debug, Into, PartialEq)]
            struct Struct<A: 'static, B: 'static> {
                field1: &'static Wrapped<A>,
                field2: &'static Wrapped<B>,
            }

            #[test]
            fn assert() {
                assert_eq!(
                    (&Wrapped(1), &Wrapped(2)),
                    Tuple(&Wrapped(1), &Wrapped(2)).into(),
                );
                assert_eq!(
                    (&Wrapped(1), &Wrapped(2)),
                    (Struct {
                        field1: &Wrapped(1),
                        field2: &Wrapped(2),
                    })
                    .into(),
                );
            }
        }

        mod bounded {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            struct Tuple<A: Clone, B: Clone>(Wrapped<A>, Wrapped<B>);

            #[derive(Debug, Into, PartialEq)]
            struct Struct<A: Clone, B: Clone> {
                field1: Wrapped<A>,
                field2: Wrapped<B>,
            }

            #[test]
            fn assert() {
                assert_eq!(
                    (Wrapped(1), Wrapped(2)),
                    Tuple(Wrapped(1), Wrapped(2)).into(),
                );
                assert_eq!(
                    (Wrapped(1), Wrapped(2)),
                    Struct {
                        field1: Wrapped(1),
                        field2: Wrapped(2),
                    }
                    .into(),
                );
            }
        }

        mod r#const {
            use super::*;

            #[derive(Debug, Into, PartialEq)]
            struct Tuple<const N: usize, A, B>(Wrapped<A>, Wrapped<B>);

            #[derive(Debug, Into, PartialEq)]
            struct Struct<A, const N: usize, B> {
                field1: Wrapped<A>,
                field2: Wrapped<B>,
            }

            #[test]
            fn assert() {
                assert_eq!(
                    (Wrapped(1), Wrapped(2)),
                    Tuple::<1, _, _>(Wrapped(1), Wrapped(2)).into(),
                );
                assert_eq!(
                    (Wrapped(1), Wrapped(2)),
                    Struct::<_, 1, _> {
                        field1: Wrapped(1),
                        field2: Wrapped(2),
                    }
                    .into(),
                );
            }
        }
    }
}

mod with_fields {
    use super::*;

    mod only {
        use super::*;

        #[derive(Clone, Copy, Debug, Into)]
        struct Tuple(#[into] i32, f64, #[into] f64);

        // Asserts that macro expansion doesn't generate this impl, by producing a trait
        // implementations conflict error during compilation, if it does.
        impl From<Tuple> for (i32, f64, f64) {
            fn from(value: Tuple) -> Self {
                (value.0, value.1, value.2)
            }
        }

        // Asserts that macro expansion doesn't generate this impl, by producing a trait
        // implementations conflict error during compilation, if it does.
        impl From<Tuple> for (i32, f64) {
            fn from(value: Tuple) -> Self {
                (value.0, value.2)
            }
        }

        #[test]
        fn tuple() {
            let t = Tuple(1, 2.0, 3.0);

            assert_eq!(1, t.into());
            assert_eq!(3.0, t.into());
        }

        #[derive(Clone, Copy, Debug, Into)]
        struct Struct {
            #[into]
            a: i32,
            b: f64,
            #[into]
            c: f64,
        }

        // Asserts that macro expansion doesn't generate this impl, by producing a trait
        // implementations conflict error during compilation, if it does.
        impl From<Struct> for (i32, f64, f64) {
            fn from(value: Struct) -> Self {
                (value.a, value.b, value.c)
            }
        }

        // Asserts that macro expansion doesn't generate this impl, by producing a trait
        // implementations conflict error during compilation, if it does.
        impl From<Struct> for (i32, f64) {
            fn from(value: Struct) -> Self {
                (value.a, value.c)
            }
        }

        #[test]
        fn named() {
            let s = Struct {
                a: 1,
                b: 2.0,
                c: 3.0,
            };

            assert_eq!(1, s.into());
            assert_eq!(3.0, s.into());
        }

        mod types {
            use super::*;

            #[derive(Clone, Debug, Into)]
            struct Tuple(
                #[into(Box<str>, Cow<'_, str>)] String,
                f64,
                #[into(f32, f64)] f32,
            );

            // Asserts that macro expansion doesn't generate this impl, by producing a trait
            // implementations conflict error during compilation, if it does.
            impl From<Tuple> for String {
                fn from(value: Tuple) -> Self {
                    value.0
                }
            }

            // Asserts that macro expansion doesn't generate this impl, by producing a trait
            // implementations conflict error during compilation, if it does.
            impl From<Tuple> for (String, f64, f32) {
                fn from(value: Tuple) -> Self {
                    (value.0, value.1, value.2)
                }
            }

            #[test]
            fn tuple() {
                let f = Tuple("1".to_owned(), 2.0, 3.0);

                assert_eq!(Box::<str>::from("1".to_owned()), f.clone().into());
                assert_eq!(Cow::Borrowed("1"), Cow::<str>::from(f.clone()));
                assert_eq!(3.0f32, f.clone().into());
                assert_eq!(3.0f64, f.into());
            }

            #[derive(Clone, Debug, Into)]
            struct Struct {
                #[into(Box<str>, Cow<'_, str>)]
                a: String,
                b: f64,
                #[into(f32, f64)]
                c: f32,
            }

            // Asserts that macro expansion doesn't generate this impl, by producing a trait
            // implementations conflict error during compilation, if it does.
            impl From<Struct> for String {
                fn from(value: Struct) -> Self {
                    value.a
                }
            }

            // Asserts that macro expansion doesn't generate this impl, by producing a trait
            // implementations conflict error during compilation, if it does.
            impl From<Struct> for (String, f64, f32) {
                fn from(value: Struct) -> Self {
                    (value.a, value.b, value.c)
                }
            }

            // Asserts that macro expansion doesn't generate this impl, by producing a trait
            // implementations conflict error during compilation, if it does.
            impl From<Struct> for (Box<str>, f32) {
                fn from(value: Struct) -> Self {
                    (value.a.into(), value.c)
                }
            }

            #[test]
            fn named() {
                let s = Struct {
                    a: "1".to_owned(),
                    b: 2.0,
                    c: 3.0,
                };

                assert_eq!(Box::<str>::from("1".to_owned()), s.clone().into());
                assert_eq!(Cow::Borrowed("1"), Cow::<str>::from(s.clone()));
                assert_eq!(3.0f32, s.clone().into());
                assert_eq!(3.0f64, s.into());
            }

            mod r#ref {
                use super::*;

                #[derive(Debug, Into)]
                struct Tuple(#[into(ref)] String, f64, #[into(ref)] f64);

                // Asserts that macro expansion doesn't generate this impl, by producing a trait
                // implementations conflict error during compilation, if it does.
                impl<'a> From<&'a Tuple> for (&'a String, &'a f64, &'a f64) {
                    fn from(value: &'a Tuple) -> Self {
                        (&value.0, &value.1, &value.2)
                    }
                }

                #[test]
                fn tuple() {
                    let t = Tuple("1".to_owned(), 2.0, 3.0);

                    assert_eq!(&"1".to_owned(), <&String>::from(&t));
                    assert_eq!(&3.0, <&f64>::from(&t));
                }

                #[derive(Debug, Into)]
                struct Struct {
                    #[into(ref)]
                    a: String,
                    b: f64,
                    #[into(ref)]
                    c: f64,
                }

                // Asserts that macro expansion doesn't generate this impl, by producing a trait
                // implementations conflict error during compilation, if it does.
                impl<'a> From<&'a Struct> for (&'a String, &'a f64, &'a f64) {
                    fn from(value: &'a Struct) -> Self {
                        (&value.a, &value.b, &value.c)
                    }
                }

                // Asserts that macro expansion doesn't generate this impl, by producing a trait
                // implementations conflict error during compilation, if it does.
                impl<'a> From<&'a Struct> for (&'a String, &'a f64) {
                    fn from(value: &'a Struct) -> Self {
                        (&value.a, &value.c)
                    }
                }

                #[test]
                fn named() {
                    let s = Struct {
                        a: "1".to_owned(),
                        b: 2.0,
                        c: 3.0,
                    };

                    assert_eq!(&"1".to_owned(), <&String>::from(&s));
                    assert_eq!(&3.0, <&f64>::from(&s));
                }

                mod types {
                    use super::*;

                    #[derive(Debug, Into)]
                    struct Tuple(
                        #[into(ref(Transmuted<i32>))] Wrapped<i32>,
                        #[into(ref(Wrapped<i64>))] Wrapped<i64>,
                    );

                    #[test]
                    fn tuple() {
                        let t = Tuple(Wrapped(1), Wrapped(2));

                        assert_eq!(&Transmuted(1), <&Transmuted<i32>>::from(&t));
                        assert_eq!(&Wrapped(2), <&Wrapped<i64>>::from(&t));
                    }

                    #[derive(Debug, Into)]
                    struct Struct {
                        #[into(ref(Transmuted<i32>))]
                        a: Wrapped<i32>,
                        #[into(ref(Wrapped<i64>))]
                        b: Wrapped<i64>,
                    }

                    #[test]
                    fn named() {
                        let s = Struct {
                            a: Wrapped(1),
                            b: Wrapped(2),
                        };

                        assert_eq!(&Transmuted(1), <&Transmuted<i32>>::from(&s));
                        assert_eq!(&Wrapped(2), <&Wrapped<i64>>::from(&s));
                    }
                }

                mod ref_mut {
                    use super::*;

                    #[derive(Debug, Into)]
                    struct Tuple(#[into(ref_mut)] i32, f64, #[into(ref_mut)] f64);

                    #[test]
                    fn tuple() {
                        let mut t = Tuple(1, 2.0, 3.0);

                        assert_eq!(&mut 1, <&mut i32>::from(&mut t));
                        assert_eq!(&mut 3.0, <&mut f64>::from(&mut t));
                    }

                    #[derive(Debug, Into)]
                    struct Struct {
                        #[into(ref_mut)]
                        a: i32,
                        b: f64,
                        #[into(ref_mut)]
                        c: f64,
                    }

                    #[test]
                    fn named() {
                        let mut s = Struct {
                            a: 1,
                            b: 2.0,
                            c: 3.0,
                        };

                        assert_eq!(&mut 1, <&mut i32>::from(&mut s));
                        assert_eq!(&mut 3.0, <&mut f64>::from(&mut s));
                    }

                    mod types {
                        use super::*;

                        #[derive(Debug, Into)]
                        struct Tuple(
                            #[into(ref_mut(Transmuted<i32>))] Wrapped<i32>,
                            #[into(ref_mut(Wrapped<i64>))] Wrapped<i64>,
                        );

                        #[test]
                        fn tuple() {
                            let mut t = Tuple(Wrapped(1), Wrapped(2));

                            assert_eq!(
                                &mut Transmuted(1),
                                <&mut Transmuted<i32>>::from(&mut t),
                            );
                            assert_eq!(
                                &mut Wrapped(2),
                                <&mut Wrapped<i64>>::from(&mut t),
                            );
                        }

                        #[derive(Debug, Into)]
                        struct Struct {
                            #[into(ref_mut(Transmuted<i32>))]
                            a: Wrapped<i32>,
                            #[into(ref_mut(Wrapped<i64>))]
                            b: Wrapped<i64>,
                        }

                        #[test]
                        fn named() {
                            let mut s = Struct {
                                a: Wrapped(1),
                                b: Wrapped(2),
                            };

                            assert_eq!(
                                &mut Transmuted(1),
                                <&mut Transmuted<i32>>::from(&mut s),
                            );
                            assert_eq!(
                                &mut Wrapped(2),
                                <&mut Wrapped<i64>>::from(&mut s),
                            );
                        }
                    }
                }
            }
        }
    }

    mod mixed {
        use super::*;

        #[derive(Debug, Into)]
        #[into(ref((Wrapped<i32>, Transmuted<f32>)))]
        struct Tuple(
            #[into(owned, ref(Transmuted<i32>))] Wrapped<i32>,
            #[into(skip)]
            #[into(ref)]
            Wrapped<f32>,
            #[into(ref_mut(Wrapped<f32>, Transmuted<f32>))] Wrapped<f32>,
        );

        #[test]
        fn tuple() {
            let mut t = Tuple(Wrapped(1), Wrapped(2.0), Wrapped(3.0));

            assert_eq!(&Transmuted(1), <&Transmuted<i32>>::from(&t));
            assert_eq!(&mut Transmuted(3.0), <&mut Transmuted<f32>>::from(&mut t));
            assert_eq!(&mut Wrapped(3.0), <&mut Wrapped<f32>>::from(&mut t));
            assert_eq!((&Wrapped(1), &Transmuted(3.0)), (&t).into());
            assert_eq!(&Wrapped(2.0), <&Wrapped<f32>>::from(&t));
            assert_eq!(Wrapped(1), t.into());
        }

        #[derive(Debug, Into)]
        #[into(ref((Wrapped<i32>, Transmuted<f32>)))]
        struct Struct {
            #[into(owned, ref(Transmuted<i32>))]
            a: Wrapped<i32>,
            #[into(skip)]
            #[into(ref)]
            b: Wrapped<f32>,
            #[into(ref_mut(Wrapped<f32>, Transmuted<f32>))]
            c: Wrapped<f32>,
        }

        #[test]
        fn named() {
            let mut s = Struct {
                a: Wrapped(1),
                b: Wrapped(2.0),
                c: Wrapped(3.0),
            };

            assert_eq!(&Transmuted(1), <&Transmuted<i32>>::from(&s));
            assert_eq!(&mut Transmuted(3.0), <&mut Transmuted<f32>>::from(&mut s));
            assert_eq!(&mut Wrapped(3.0), <&mut Wrapped<f32>>::from(&mut s));
            assert_eq!((&Wrapped(1), &Transmuted(3.0)), (&s).into());
            assert_eq!(&Wrapped(2.0), <&Wrapped<f32>>::from(&s));
            assert_eq!(Wrapped(1), s.into());
        }

        mod separate {
            use super::*;

            #[derive(Clone, Copy, Debug, Into)]
            #[into(ref)]
            #[into(owned)]
            #[into((Wrapped<i32>, Transmuted<f32>))]
            #[into(ref_mut((Wrapped<i32>, Transmuted<f32>)))]
            struct Tuple(
                #[into(ref)]
                #[into(ref(Transmuted<i32>))]
                #[into]
                Wrapped<i32>,
                #[into(ref_mut)]
                #[into(ref_mut(Transmuted<f32>))]
                #[into(owned)]
                Wrapped<f32>,
            );

            #[test]
            fn tuple() {
                let mut t = Tuple(Wrapped(1), Wrapped(2.0));

                assert_eq!((&Wrapped(1), &Wrapped(2.0)), (&t).into());
                assert_eq!((Wrapped(1), Wrapped(2.0)), t.into());
                assert_eq!((Wrapped(1), Transmuted(2.0)), t.into());
                assert_eq!((&mut Wrapped(1), &mut Transmuted(2.0)), (&mut t).into());
                assert_eq!(&Wrapped(1), <&Wrapped<i32>>::from(&t));
                assert_eq!(&Transmuted(1), <&Transmuted<i32>>::from(&t));
                assert_eq!(Wrapped(1), <Wrapped<i32>>::from(t));
                assert_eq!(&mut Wrapped(2.0), <&mut Wrapped<f32>>::from(&mut t));
                assert_eq!(&mut Transmuted(2.0), <&mut Transmuted<f32>>::from(&mut t));
                assert_eq!(Wrapped(2.0), <Wrapped<f32>>::from(t));
            }

            #[derive(Clone, Copy, Debug, Into)]
            #[into(ref)]
            #[into(owned)]
            #[into((Wrapped<i32>, Transmuted<f32>))]
            #[into(ref_mut((Wrapped<i32>, Transmuted<f32>)))]
            struct Struct {
                #[into(ref)]
                #[into(ref (Transmuted < i32 >))]
                #[into]
                a: Wrapped<i32>,
                #[into(ref_mut)]
                #[into(ref_mut(Transmuted < f32 >))]
                #[into(owned)]
                b: Wrapped<f32>,
            }

            #[test]
            fn named() {
                let mut s = Struct {
                    a: Wrapped(1),
                    b: Wrapped(2.0),
                };

                assert_eq!((&Wrapped(1), &Wrapped(2.0)), (&s).into());
                assert_eq!((Wrapped(1), Wrapped(2.0)), s.into());
                assert_eq!((Wrapped(1), Transmuted(2.0)), s.into());
                assert_eq!((&mut Wrapped(1), &mut Transmuted(2.0)), (&mut s).into());
                assert_eq!(&Wrapped(1), <&Wrapped<i32>>::from(&s));
                assert_eq!(&Transmuted(1), <&Transmuted<i32>>::from(&s));
                assert_eq!(Wrapped(1), <Wrapped<i32>>::from(s));
                assert_eq!(&mut Wrapped(2.0), <&mut Wrapped<f32>>::from(&mut s));
                assert_eq!(&mut Transmuted(2.0), <&mut Transmuted<f32>>::from(&mut s),);
                assert_eq!(Wrapped(2.0), <Wrapped<f32>>::from(s));
            }
        }
    }
}
