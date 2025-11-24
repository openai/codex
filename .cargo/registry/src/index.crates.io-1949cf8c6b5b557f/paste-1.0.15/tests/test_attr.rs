#![allow(clippy::let_underscore_untyped)]

use paste::paste;
use paste_test_suite::paste_test;

#[test]
fn test_attr() {
    paste! {
        #[paste_test(k = "val" "ue")]
        struct A;

        #[paste_test_suite::paste_test(k = "val" "ue")]
        struct B;

        #[::paste_test_suite::paste_test(k = "val" "ue")]
        struct C;

        #[paste_test(k = "va" [<l u>] e)]
        struct D;
    }

    let _ = A;
    let _ = B;
    let _ = C;
    let _ = D;
}

#[test]
fn test_paste_cfg() {
    macro_rules! m {
        ($ret:ident, $width:expr) => {
            paste! {
                #[cfg(any(feature = "protocol_feature_" $ret:snake, target_pointer_width = "" $width))]
                fn new() -> $ret { todo!() }
            }
        };
    }

    struct Paste;

    #[cfg(target_pointer_width = "64")]
    m!(Paste, 64);
    #[cfg(target_pointer_width = "32")]
    m!(Paste, 32);

    let _ = new;
}

#[test]
fn test_path_in_attr() {
    macro_rules! m {
        (#[x = $x:ty]) => {
            stringify!($x)
        };
    }

    let ty = paste! {
        m!(#[x = foo::Bar])
    };

    assert_eq!("foo::Bar", ty);
}
