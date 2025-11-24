#![warn(rust_2018_idioms)]
#![cfg(feature = "sync")]

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn is_error<T: std::error::Error + Send + Sync>() {}

#[test]
fn mpsc_error_bound() {
    use tokio::sync::mpsc::error;

    is_error::<error::SendError<()>>();
    is_error::<error::TrySendError<()>>();
}

#[test]
fn oneshot_error_bound() {
    use tokio::sync::oneshot::error;

    is_error::<error::RecvError>();
    is_error::<error::TryRecvError>();
}

#[test]
fn watch_error_bound() {
    use tokio::sync::watch::error;

    is_error::<error::SendError<()>>();
}
