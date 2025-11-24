#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support threading

use tokio::test;

#[test]
async fn test_macro_can_be_used_via_use() {
    tokio::spawn(async {}).await.unwrap();
}

#[tokio::test]
async fn test_macro_is_resilient_to_shadowing() {
    tokio::spawn(async {}).await.unwrap();
}

// https://github.com/tokio-rs/tokio/issues/3403
#[rustfmt::skip] // this `rustfmt::skip` is necessary because unused_braces does not warn if the block contains newline.
#[tokio::main]
pub async fn unused_braces_main() { println!("hello") }
#[rustfmt::skip] // this `rustfmt::skip` is necessary because unused_braces does not warn if the block contains newline.
#[tokio::test]
async fn unused_braces_test() { assert_eq!(1 + 1, 2) }

// https://github.com/tokio-rs/tokio/pull/3766#issuecomment-835508651
#[std::prelude::v1::test]
fn trait_method() {
    trait A {
        fn f(self);

        fn g(self);
    }
    impl A for () {
        #[tokio::main]
        async fn f(self) {
            self.g()
        }

        fn g(self) {}
    }
    ().f()
}

// https://github.com/tokio-rs/tokio/issues/4175
#[tokio::main]
pub async fn issue_4175_main_1() -> ! {
    panic!();
}
#[tokio::main]
pub async fn issue_4175_main_2() -> std::io::Result<()> {
    panic!();
}
#[allow(unreachable_code)]
#[tokio::test]
pub async fn issue_4175_test() -> std::io::Result<()> {
    return Ok(());
    panic!();
}

// https://github.com/tokio-rs/tokio/issues/4175
#[allow(clippy::let_unit_value)]
pub mod clippy_semicolon_if_nothing_returned {
    #![deny(clippy::semicolon_if_nothing_returned)]

    #[tokio::main]
    pub async fn local() {
        let _x = ();
    }
    #[tokio::main]
    pub async fn item() {
        fn _f() {}
    }
    #[tokio::main]
    pub async fn semi() {
        panic!();
    }
    #[tokio::main]
    pub async fn empty() {
        // To trigger clippy::semicolon_if_nothing_returned lint, the block needs to contain newline.
    }
}

// https://github.com/tokio-rs/tokio/issues/5243
pub mod issue_5243 {
    macro_rules! mac {
        (async fn $name:ident() $b:block) => {
            #[::tokio::test]
            async fn $name() {
                $b
            }
        };
    }
    mac!(
        async fn foo() {}
    );
}

#[cfg(tokio_unstable)]
pub mod macro_rt_arg_unhandled_panic {
    use tokio_test::assert_err;

    #[tokio::test(flavor = "current_thread", unhandled_panic = "shutdown_runtime")]
    #[should_panic]
    async fn unhandled_panic_shutdown_runtime() {
        let _ = tokio::spawn(async {
            panic!("This panic should shutdown the runtime.");
        })
        .await;
    }

    #[tokio::test(flavor = "current_thread", unhandled_panic = "ignore")]
    async fn unhandled_panic_ignore() {
        let rt = tokio::spawn(async {
            panic!("This panic should be forwarded to rt as an error.");
        })
        .await;
        assert_err!(rt);
    }
}
