#![cfg(not(feature = "full"))]
compile_error!("run tokio-util tests with `--features full`");
