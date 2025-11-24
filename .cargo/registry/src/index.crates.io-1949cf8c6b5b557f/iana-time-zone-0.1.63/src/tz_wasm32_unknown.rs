use js_sys::{Array, Intl, Object, Reflect};
use wasm_bindgen::JsValue;

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    let intl = Intl::DateTimeFormat::new(&Array::new(), &Object::new()).resolved_options();
    Reflect::get(&intl, &JsValue::from_str("timeZone"))
        .ok()
        .and_then(|tz| tz.as_string())
        .ok_or(crate::GetTimezoneError::OsError)
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn pass() {
        let tz = super::get_timezone_inner().unwrap();
        console_log!("tz={:?}", tz);
    }
}
