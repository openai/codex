#[cfg(not(any(feature = "full", target_family = "wasm")))]
compile_error!("run main Tokio tests with `--features full`");

// CI sets `--cfg tokio_no_parking_lot` when trying to run tests with
// `parking_lot` disabled. This check prevents "silent failure" if `parking_lot`
// accidentally gets enabled.
#[cfg(all(tokio_no_parking_lot, feature = "parking_lot"))]
compile_error!("parking_lot feature enabled when it should not be");
