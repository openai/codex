#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support panic recovery
#![cfg(panic = "unwind")]

struct PanicsOnDrop;

impl Drop for PanicsOnDrop {
    fn drop(&mut self) {
        panic!("I told you so");
    }
}

#[tokio::test]
async fn test_panics_do_not_propagate_when_dropping_join_handle() {
    let join_handle = tokio::spawn(async move { PanicsOnDrop });

    // only drop the JoinHandle when the task has completed
    // (which is difficult to synchronize precisely)
    tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    drop(join_handle);
}
