#![allow(clippy::let_underscore_untyped)]

mod test_basic {
    use paste::paste;

    struct Struct;

    paste! {
        impl Struct {
            fn [<a b c>]() {}
        }
    }

    #[test]
    fn test() {
        Struct::abc();
    }
}

mod test_in_impl {
    use paste::paste;

    struct Struct;

    impl Struct {
        paste! {
            fn [<a b c>]() {}
        }
    }

    #[test]
    fn test() {
        Struct::abc();
    }
}

mod test_none_delimited_single_ident {
    use paste::paste;

    macro_rules! m {
        ($id:ident) => {
            paste! {
                fn f() -> &'static str {
                    stringify!($id)
                }
            }
        };
    }

    m!(i32x4);

    #[test]
    fn test() {
        assert_eq!(f(), "i32x4");
    }
}

mod test_none_delimited_single_lifetime {
    use paste::paste;

    macro_rules! m {
        ($life:lifetime) => {
            paste! {
                pub struct S<$life>(#[allow(dead_code)] pub &$life ());
                impl<$life> S<$life> {
                    fn f() {}
                }
            }
        };
    }

    m!('a);

    #[test]
    fn test() {
        S::f();
    }
}

mod test_to_lower {
    use paste::paste;

    macro_rules! m {
        ($id:ident) => {
            paste! {
                fn [<my_ $id:lower _here>](_arg: u8) -> &'static str {
                    stringify!([<$id:lower>])
                }
            }
        };
    }

    m!(Test);

    #[test]
    fn test_to_lower() {
        assert_eq!(my_test_here(0), "test");
    }
}

mod test_to_upper {
    use paste::paste;

    macro_rules! m {
        ($id:ident) => {
            paste! {
                const [<MY_ $id:upper _HERE>]: &str = stringify!([<$id:upper>]);
            }
        };
    }

    m!(Test);

    #[test]
    fn test_to_upper() {
        assert_eq!(MY_TEST_HERE, "TEST");
    }
}

mod test_to_snake {
    use paste::paste;

    macro_rules! m {
        ($id:ident) => {
            paste! {
                const DEFAULT_SNAKE: &str = stringify!([<$id:snake>]);
                const LOWER_SNAKE: &str = stringify!([<$id:snake:lower>]);
                const UPPER_SNAKE: &str = stringify!([<$id:snake:upper>]);
            }
        };
    }

    m!(ThisIsButATest);

    #[test]
    fn test_to_snake() {
        assert_eq!(DEFAULT_SNAKE, "this_is_but_a_test");
        assert_eq!(LOWER_SNAKE, "this_is_but_a_test");
        assert_eq!(UPPER_SNAKE, "THIS_IS_BUT_A_TEST");
    }
}

mod test_to_camel {
    use paste::paste;

    macro_rules! m {
        ($id:ident) => {
            paste! {
                const DEFAULT_CAMEL: &str = stringify!([<$id:camel>]);
                const LOWER_CAMEL: &str = stringify!([<$id:camel:lower>]);
                const UPPER_CAMEL: &str = stringify!([<$id:camel:upper>]);
            }
        };
    }

    m!(this_is_but_a_test);

    #[test]
    fn test_to_camel() {
        assert_eq!(DEFAULT_CAMEL, "ThisIsButATest");
        assert_eq!(LOWER_CAMEL, "thisisbutatest");
        assert_eq!(UPPER_CAMEL, "THISISBUTATEST");
    }
}

mod test_doc_expr {
    // https://github.com/dtolnay/paste/issues/29

    use paste::paste;

    macro_rules! doc_expr {
        ($doc:expr) => {
            paste! {
                #[doc = $doc]
                pub struct S;
            }
        };
    }

    doc_expr!(stringify!(...));

    #[test]
    fn test_doc_expr() {
        let _: S;
    }
}

mod test_type_in_path {
    // https://github.com/dtolnay/paste/issues/31

    use paste::paste;

    mod keys {
        #[derive(Default)]
        pub struct Mib<T = ()>(std::marker::PhantomData<T>);
    }

    macro_rules! types {
        ($mib:ty) => {
            paste! {
                #[derive(Default)]
                pub struct S(pub keys::$mib);
            }
        };
    }

    macro_rules! write {
        ($fn:ident, $field:ty) => {
            paste! {
                pub fn $fn() -> $field {
                    $field::default()
                }
            }
        };
    }

    types! {Mib<[usize; 2]>}
    write! {get_a, keys::Mib}
    write! {get_b, usize}

    #[test]
    fn test_type_in_path() {
        let _: S;
        let _ = get_a;
        let _ = get_b;
    }
}

mod test_type_in_fn_arg {
    // https://github.com/dtolnay/paste/issues/38

    use paste::paste;

    fn _jit_address(_node: ()) {}

    macro_rules! jit_reexport {
        ($fn:ident, $arg:ident : $typ:ty) => {
            paste! {
                pub fn $fn($arg: $typ) {
                    [<_jit_ $fn>]($arg);
                }
            }
        };
    }

    jit_reexport!(address, node: ());

    #[test]
    fn test_type_in_fn_arg() {
        let _ = address;
    }
}

mod test_pat_in_expr_position {
    // https://github.com/xiph/rav1e/pull/2324/files

    use paste::paste;

    macro_rules! rav1e_bad {
        ($e:pat) => {
            paste! {
                #[test]
                fn test() {
                    let _ = $e;
                }
            }
        };
    }

    rav1e_bad!(std::fmt::Error);
}
