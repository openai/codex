#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use tokio::io::AsyncRead;

#[test]
fn assert_obj_safe() {
    fn _assert<T>() {}
    _assert::<Box<dyn AsyncRead>>();
}
