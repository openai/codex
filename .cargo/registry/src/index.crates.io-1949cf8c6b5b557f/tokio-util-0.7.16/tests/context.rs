#![cfg(feature = "rt")]
#![cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#![warn(rust_2018_idioms)]

use tokio::runtime::Builder;
use tokio::time::*;
use tokio_util::context::RuntimeExt;

#[test]
fn tokio_context_with_another_runtime() {
    let rt1 = Builder::new_multi_thread()
        .worker_threads(1)
        // no timer!
        .build()
        .unwrap();
    let rt2 = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    // Without the `HandleExt.wrap()` there would be a panic because there is
    // no timer running, since it would be referencing runtime r1.
    rt1.block_on(rt2.wrap(async move { sleep(Duration::from_millis(2)).await }));
}
