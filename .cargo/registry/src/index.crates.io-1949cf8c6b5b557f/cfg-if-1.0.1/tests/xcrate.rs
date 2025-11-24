#![allow(unexpected_cfgs)] // `foo` doesn't exist

cfg_if::cfg_if! {
    if #[cfg(foo)] {
        fn works() -> bool { false }
    } else if #[cfg(test)] {
        fn works() -> bool { true }
    } else {
        fn works() -> bool { false }
    }
}

#[test]
fn smoke() {
    assert!(works());
}
