#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support threading

#[allow(unused_imports)]
use std as tokio;

use ::tokio as tokio1;

mod test {
    pub use ::tokio;
}

async fn compute() -> usize {
    let join = tokio1::spawn(async { 1 });
    join.await.unwrap()
}

#[tokio1::main(crate = "tokio1")]
async fn compute_main() -> usize {
    compute().await
}

#[test]
fn crate_rename_main() {
    assert_eq!(1, compute_main());
}

#[tokio1::test(crate = "tokio1")]
async fn crate_rename_test() {
    assert_eq!(1, compute().await);
}

#[test::tokio::test(crate = "test::tokio")]
async fn crate_path_test() {
    assert_eq!(1, compute().await);
}
