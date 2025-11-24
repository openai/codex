//! Separate test binary, because the panic hook is a global resource

use std::{
    panic::{catch_unwind, set_hook, take_hook},
    path::Path,
    sync::OnceLock,
};

use axum::{routing::get, Router};

#[test]
fn routes_with_overlapping_method_routes() {
    static PANIC_LOCATION_FILE: OnceLock<String> = OnceLock::new();

    let default_hook = take_hook();
    set_hook(Box::new(|panic_info| {
        if let Some(location) = panic_info.location() {
            _ = PANIC_LOCATION_FILE.set(location.file().to_owned());
        }
    }));

    let result = catch_unwind(|| {
        async fn handler() {}

        let _: Router = Router::new()
            .route("/foo/bar", get(handler))
            .route("/foo/bar", get(handler));
    });
    set_hook(default_hook);

    let panic_payload = result.unwrap_err();
    let panic_msg = panic_payload.downcast_ref::<String>().unwrap();

    assert_eq!(
        panic_msg,
        "Overlapping method route. Handler for `GET /foo/bar` already exists"
    );

    let file = PANIC_LOCATION_FILE.get().unwrap();
    assert_eq!(Path::new(file).file_name().unwrap(), "panic_location.rs");
}
