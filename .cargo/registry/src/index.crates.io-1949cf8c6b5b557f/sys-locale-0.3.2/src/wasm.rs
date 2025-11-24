use alloc::string::String;

use js_sys::{JsString, Object};
use wasm_bindgen::{prelude::*, JsCast, JsValue};

#[derive(Clone)]
enum GlobalType {
    Window(web_sys::Window),
    Worker(web_sys::WorkerGlobalScope),
}

/// Returns a handle to the global scope object.
///
/// Simplified version of https://github.com/rustwasm/wasm-bindgen/blob/main/crates/js-sys/src/lib.rs,
/// which we can't use directly because it discards information about how it
/// retrieved the global.
fn global() -> GlobalType {
    #[wasm_bindgen]
    extern "C" {
        type Global;

        #[wasm_bindgen(getter, catch, static_method_of = Global, js_class = window, js_name = window)]
        fn get_window() -> Result<Object, JsValue>;

        #[wasm_bindgen(getter, catch, static_method_of = Global, js_class = self, js_name = self)]
        fn get_self() -> Result<Object, JsValue>;
    }

    if let Ok(window) = Global::get_window() {
        GlobalType::Window(
            window
                .dyn_into::<web_sys::Window>()
                .expect("expected window to be an instance of Window"),
        )
    } else if let Ok(worker) = Global::get_self() {
        GlobalType::Worker(
            worker
                .dyn_into::<web_sys::WorkerGlobalScope>()
                .expect("expected self to be an instance of WorkerGlobalScope"),
        )
    } else {
        panic!("Unable to find global in this environment")
    }
}

pub(crate) fn get() -> impl Iterator<Item = String> {
    let languages = match global() {
        GlobalType::Window(window) => window.navigator().languages(),
        GlobalType::Worker(worker) => worker.navigator().languages(),
    };
    languages
        .values()
        .into_iter()
        .flat_map(|v| v.and_then(|v| v.dyn_into::<JsString>()))
        .map(String::from)
}
