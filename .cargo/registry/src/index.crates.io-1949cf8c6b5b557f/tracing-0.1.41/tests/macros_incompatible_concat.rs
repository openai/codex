use tracing::{enabled, event, span, Level};

#[macro_export]
macro_rules! concat {
    () => {};
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn span() {
    span!(Level::DEBUG, "foo");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn event() {
    event!(Level::DEBUG, "foo");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn enabled() {
    enabled!(Level::DEBUG);
}
