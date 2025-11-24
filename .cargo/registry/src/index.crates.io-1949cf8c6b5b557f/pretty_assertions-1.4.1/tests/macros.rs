#![cfg_attr(not(feature = "std"), no_std)]
#![no_implicit_prelude]

#[cfg(feature = "alloc")]
extern crate alloc;

#[allow(clippy::eq_op)]
mod assert_str_eq {
    use ::core::{cmp::PartialEq, convert::AsRef};

    #[cfg(feature = "alloc")]
    use ::alloc::string::{String, ToString};
    #[cfg(feature = "std")]
    use ::std::string::{String, ToString};

    #[test]
    fn passes_str() {
        let a = "some value";
        ::pretty_assertions::assert_str_eq!(a, a);
    }

    #[test]
    fn passes_string() {
        let a: String = "some value".to_string();
        ::pretty_assertions::assert_str_eq!(a, a);
    }

    #[test]
    fn passes_comparable_types() {
        let s0: &'static str = "foo";
        let s1: String = "foo".to_string();
        ::pretty_assertions::assert_str_eq!(s0, s1);
    }

    #[derive(PartialEq)]
    struct MyString(String);

    impl AsRef<str> for MyString {
        fn as_ref(&self) -> &str {
            &self.0
        }
    }

    impl PartialEq<String> for MyString {
        fn eq(&self, other: &String) -> bool {
            &self.0 == other
        }
    }

    #[test]
    fn passes_as_ref_types() {
        let s0 = MyString("foo".to_string());
        let s1 = "foo".to_string();
        ::pretty_assertions::assert_str_eq!(s0, s1);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left == right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
 foo
[31m<ba[0m[1;48;5;52;31mr[0m
[32m>ba[0m[1;48;5;22;32mz[0m

"#)]
    fn fails_as_ref_types() {
        let s0 = MyString("foo\nbar".to_string());
        let s1 = "foo\nbaz".to_string();
        ::pretty_assertions::assert_str_eq!(s0, s1);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left == right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
 foo
[31m<ba[0m[1;48;5;52;31mr[0m
[32m>ba[0m[1;48;5;22;32mz[0m

"#)]
    fn fails_foo() {
        ::pretty_assertions::assert_str_eq!("foo\nbar", "foo\nbaz");
    }
}

#[allow(clippy::eq_op)]
mod assert_eq {
    #[cfg(feature = "alloc")]
    use ::alloc::string::{String, ToString};
    #[cfg(feature = "std")]
    use ::std::string::{String, ToString};

    #[test]
    fn passes() {
        let a = "some value";
        ::pretty_assertions::assert_eq!(a, a);
    }

    #[test]
    fn passes_unsized() {
        let a: &[u8] = b"e";
        ::pretty_assertions::assert_eq!(*a, *a);
    }

    #[test]
    fn passes_comparable_types() {
        let s0: &'static str = "foo";
        let s1: String = "foo".to_string();
        ::pretty_assertions::assert_eq!(s0, s1);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left == right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
[31m<[0m[1;48;5;52;31m666[0m
[32m>[0m[1;48;5;22;32m999[0m

"#)]
    fn fails() {
        ::pretty_assertions::assert_eq!(666, 999);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left == right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
[31m<[0m[1;48;5;52;31m666[0m
[32m>[0m[1;48;5;22;32m999[0m

"#)]
    fn fails_trailing_comma() {
        ::pretty_assertions::assert_eq!(666, 999,);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left == right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
 [
     101,
[32m>    101,[0m
 ]

"#)]
    fn fails_unsized() {
        let a: &[u8] = b"e";
        let b: &[u8] = b"ee";
        ::pretty_assertions::assert_eq!(*a, *b);
    }

    #[test]
    #[should_panic(
        expected = r#"assertion failed: `(left == right)`: custom panic message

[1mDiff[0m [31m< left[0m / [32mright >[0m :
[31m<[0m[1;48;5;52;31m666[0m
[32m>[0m[1;48;5;22;32m999[0m

"#
    )]
    fn fails_custom() {
        ::pretty_assertions::assert_eq!(666, 999, "custom panic message");
    }

    #[test]
    #[should_panic(
        expected = r#"assertion failed: `(left == right)`: custom panic message

[1mDiff[0m [31m< left[0m / [32mright >[0m :
[31m<[0m[1;48;5;52;31m666[0m
[32m>[0m[1;48;5;22;32m999[0m

"#
    )]
    fn fails_custom_trailing_comma() {
        ::pretty_assertions::assert_eq!(666, 999, "custom panic message",);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left == right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
 foo
[31m<ba[0m[1;48;5;52;31mr[0m
[32m>ba[0m[1;48;5;22;32mz[0m

"#)]
    fn fails_str() {
        ::pretty_assertions::assert_eq!("foo\nbar", "foo\nbaz");
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left == right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
 foo
[31m<ba[0m[1;48;5;52;31mr[0m
[32m>ba[0m[1;48;5;22;32mz[0m

"#)]
    fn fails_string() {
        ::pretty_assertions::assert_eq!("foo\nbar".to_string(), "foo\nbaz".to_string());
    }
}

mod assert_ne {
    #[cfg(feature = "alloc")]
    use ::alloc::string::{String, ToString};
    #[cfg(feature = "std")]
    use ::std::string::{String, ToString};

    #[test]
    fn passes() {
        let a = "a";
        let b = "b";
        ::pretty_assertions::assert_ne!(a, b);
    }

    #[test]
    fn passes_unsized() {
        let a: &[u8] = b"e";
        let b: &[u8] = b"ee";
        ::pretty_assertions::assert_ne!(*a, *b);
    }

    #[test]
    fn passes_comparable_types() {
        let s0: &'static str = "foo";
        let s1: String = "bar".to_string();
        ::pretty_assertions::assert_ne!(s0, s1);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left != right)`

Both sides:
666
"#)]
    fn fails() {
        ::pretty_assertions::assert_ne!(666, 666);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left != right)`

Both sides:
666
"#)]
    fn fails_trailing_comma() {
        ::pretty_assertions::assert_ne!(666, 666,);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left != right)`

Both sides:
[
    101,
]

"#)]
    fn fails_unsized() {
        let a: &[u8] = b"e";
        ::pretty_assertions::assert_ne!(*a, *a);
    }

    #[test]
    #[should_panic(
        expected = r#"assertion failed: `(left != right)`: custom panic message

Both sides:
666
"#
    )]
    fn fails_custom() {
        ::pretty_assertions::assert_ne!(666, 666, "custom panic message");
    }

    #[test]
    #[should_panic(
        expected = r#"assertion failed: `(left != right)`: custom panic message

Both sides:
666
"#
    )]
    fn fails_custom_trailing_comma() {
        ::pretty_assertions::assert_ne!(666, 666, "custom panic message",);
    }

    // If the values are equal but their debug outputs are not
    // show a specific warning

    // Regression tests

    #[test]
    #[should_panic]
    fn assert_ne_non_empty_return() {
        fn not_zero(x: u32) -> u32 {
            ::pretty_assertions::assert_ne!(x, 0);
            x
        }
        not_zero(0);
    }
}

#[cfg(feature = "unstable")]
mod assert_matches {
    use ::core::option::Option::{None, Some};

    #[test]
    fn passes() {
        let a = Some("some value");
        ::pretty_assertions::assert_matches!(a, Some(_));
    }

    #[test]
    fn passes_unsized() {
        let a: &[u8] = b"e";
        ::pretty_assertions::assert_matches!(*a, _);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left matches right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
[31m<[0m[1;48;5;52;31mN[0m[31mo[0m[1;48;5;52;31mn[0m[31me[0m
[32m>[0m[1;48;5;22;32mS[0m[32mo[0m[1;48;5;22;32mm[0m[32me[0m[1;48;5;22;32m(_)[0m

"#)]
    fn fails() {
        ::pretty_assertions::assert_matches!(None::<usize>, Some(_));
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left matches right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
[31m<Some([0m
[31m<    3,[0m
[31m<)[0m
[32m>Some(3) if 0 > 0[0m

"#)]
    fn fails_guard() {
        ::pretty_assertions::assert_matches!(Some(3), Some(3) if 0 > 0,);
    }

    #[test]
    #[should_panic(expected = r#"assertion failed: `(left matches right)`

[1mDiff[0m [31m< left[0m / [32mright >[0m :
[31m<[[0m
[31m<    101,[0m
[31m<][0m
[32m>ref b if b == b"ee"[0m

"#)]
    fn fails_unsized() {
        let a: &[u8] = b"e";
        ::pretty_assertions::assert_matches!(*a, ref b if b == b"ee");
    }

    #[test]
    #[should_panic(
        expected = r#"assertion failed: `(left matches right)`: custom panic message

[1mDiff[0m [31m< left[0m / [32mright >[0m :
[31m<[0m[1;48;5;52;31m666[0m
[32m>[0m[1;48;5;22;32m999[0m

"#
    )]
    fn fails_custom() {
        ::pretty_assertions::assert_matches!(666, 999, "custom panic message");
    }

    #[test]
    #[should_panic(
        expected = r#"assertion failed: `(left matches right)`: custom panic message

[1mDiff[0m [31m< left[0m / [32mright >[0m :
[31m<[0m[1;48;5;52;31m666[0m
[32m>[0m[1;48;5;22;32m999[0m

"#
    )]
    fn fails_custom_trailing_comma() {
        ::pretty_assertions::assert_matches!(666, 999, "custom panic message",);
    }
}
