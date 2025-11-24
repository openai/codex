#![allow(clippy::let_underscore_untyped)]

use paste::paste;

#[test]
fn test_shared_hygiene() {
    paste! {
        let [<a a>] = 1;
        assert_eq!([<a a>], 1);
    }
}

#[test]
fn test_repeat() {
    const ROCKET_A: &str = "/a";
    const ROCKET_B: &str = "/b";

    macro_rules! routes {
        ($($route:ident),*) => {{
            paste! {
                vec![$( [<ROCKET_ $route>] ),*]
            }
        }}
    }

    let routes = routes!(A, B);
    assert_eq!(routes, vec!["/a", "/b"]);
}

#[test]
fn test_literal_to_identifier() {
    const CONST0: &str = "const0";

    let pasted = paste!([<CONST 0>]);
    assert_eq!(pasted, CONST0);

    let pasted = paste!([<CONST '0'>]);
    assert_eq!(pasted, CONST0);

    let pasted = paste!([<CONST "0">]);
    assert_eq!(pasted, CONST0);

    let pasted = paste!([<CONST r"0">]);
    assert_eq!(pasted, CONST0);

    let pasted = paste!([<CONST '\u{30}'>]);
    assert_eq!(pasted, CONST0);
}

#[test]
fn test_literal_suffix() {
    macro_rules! literal {
        ($bit:tt) => {
            paste!([<1_u $bit>])
        };
    }

    assert_eq!(literal!(32), 1);
}

#[test]
fn test_underscore() {
    paste! {
        const A_B: usize = 0;
        assert_eq!([<A _ B>], 0);
    }
}

#[test]
fn test_lifetime() {
    paste! {
        #[allow(dead_code)]
        struct S<[<'d e>]> {
            q: &[<'d e>] str,
        }
    }
}

#[test]
fn test_keyword() {
    paste! {
        struct [<F move>];

        let _ = Fmove;
    }
}

#[test]
fn test_literal_str() {
    paste! {
        #[allow(non_camel_case_types)]
        struct [<Foo "Bar-Baz">];

        let _ = FooBar_Baz;
    }
}

#[test]
fn test_env_literal() {
    paste! {
        struct [<Lib env bar>];

        let _ = Libenvbar;
    }
}

#[test]
fn test_env_present() {
    paste! {
        struct [<Lib env!("CARGO_PKG_NAME")>];

        let _ = Libpaste;
    }
}

#[test]
fn test_raw_identifier() {
    paste! {
        struct [<F r#move>];

        let _ = Fmove;
    }
}

#[test]
fn test_false_start() {
    trait Trait {
        fn f() -> usize;
    }

    struct S;

    impl Trait for S {
        fn f() -> usize {
            0
        }
    }

    paste! {
        let x = [<S as Trait>::f()];
        assert_eq!(x[0], 0);
    }
}

#[test]
fn test_local_variable() {
    let yy = 0;

    paste! {
        assert_eq!([<y y>], 0);
    }
}

#[test]
fn test_empty() {
    paste! {
        assert_eq!(stringify!([<y y>]), "yy");
        assert_eq!(stringify!([<>]).replace(' ', ""), "[<>]");
    }
}

#[test]
fn test_env_to_lower() {
    paste! {
        struct [<Lib env!("CARGO_PKG_NAME"):lower>];

        let _ = Libpaste;
    }
}

#[test]
fn test_env_to_upper() {
    paste! {
        const [<LIB env!("CARGO_PKG_NAME"):upper>]: &str = "libpaste";

        let _ = LIBPASTE;
    }
}

#[test]
fn test_env_to_snake() {
    paste! {
        const [<LIB env!("CARGO_PKG_NAME"):snake:upper>]: &str = "libpaste";

        let _ = LIBPASTE;
    }
}

#[test]
fn test_env_to_camel() {
    paste! {
        #[allow(non_upper_case_globals)]
        const [<LIB env!("CARGO_PKG_NAME"):camel>]: &str = "libpaste";

        let _ = LIBPaste;
    }
}

mod test_x86_feature_literal {
    // work around https://github.com/rust-lang/rust/issues/72726

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    macro_rules! my_is_x86_feature_detected {
        ($feat:literal) => {
            use paste::paste;

            paste! {
                #[test]
                fn test() {
                    let _ = is_x86_feature_detected!($feat);
                }
            }
        };
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    macro_rules! my_is_x86_feature_detected {
        ($feat:literal) => {
            #[ignore]
            #[test]
            fn test() {}
        };
    }

    my_is_x86_feature_detected!("mmx");
}

#[rustversion::since(1.46)]
mod test_local_setter {
    // https://github.com/dtolnay/paste/issues/7

    use paste::paste;

    #[derive(Default)]
    struct Test {
        val: i32,
    }

    impl Test {
        fn set_val(&mut self, arg: i32) {
            self.val = arg;
        }
    }

    macro_rules! setter {
        ($obj:expr, $field:ident, $value:expr) => {
            paste! { $obj.[<set_ $field>]($value); }
        };

        ($field:ident, $value:expr) => {{
            let mut new = Test::default();
            setter!(new, val, $value);
            new
        }};
    }

    #[test]
    fn test_local_setter() {
        let a = setter!(val, 42);
        assert_eq!(a.val, 42);
    }
}

// https://github.com/dtolnay/paste/issues/85
#[test]
fn test_top_level_none_delimiter() {
    macro_rules! clone {
        ($val:expr) => {
            paste! {
                $val.clone()
            }
        };
    }

    #[derive(Clone)]
    struct A;

    impl A {
        fn consume_self(self) {
            let _ = self;
        }
    }

    clone!(&A).consume_self();
}
