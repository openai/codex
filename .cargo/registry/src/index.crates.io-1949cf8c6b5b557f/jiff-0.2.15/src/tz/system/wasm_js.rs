use alloc::string::String;

use crate::tz::{TimeZone, TimeZoneDatabase};

pub(super) fn get(db: &TimeZoneDatabase) -> Option<TimeZone> {
    let fmt = js_sys::Intl::DateTimeFormat::new(
        &js_sys::Array::new(),
        &js_sys::Object::new(),
    );
    let options = fmt.resolved_options();
    // Documented to be an IANA tz ID:
    // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/DateTimeFormat/resolvedOptions#timezone
    let key = wasm_bindgen::JsValue::from("timeZone");
    let val = match js_sys::Reflect::get(&options, &key) {
        Ok(val) => val,
        Err(_err) => {
            trace!(
                "failed to get `timeZone` key on \
                 Intl.DateTimeFormat options: {_err:?}"
            );
            return None;
        }
    };
    trace!("got `timeZone` value from Intl.DateTimeFormat options: {val:?}");
    let name = match String::try_from(val) {
        Ok(string) => string,
        Err(_err) => {
            trace!(
                "failed to convert `timeZone` on \
                 Intl.DateTimeFormat to string"
            );
            return None;
        }
    };
    let tz = match db.get(&name) {
        Ok(tz) => tz,
        Err(_err) => {
            trace!(
                "got {name:?} as time zone name, \
                 but failed to find time zone with that name in \
                 zoneinfo database {db:?}",
            );
            return None;
        }
    };
    Some(tz)
}

pub(super) fn read(_db: &TimeZoneDatabase, path: &str) -> Option<TimeZone> {
    match super::read_unnamed_tzif_file(path) {
        Ok(tz) => Some(tz),
        Err(_err) => {
            trace!("failed to read {path} as unnamed time zone: {_err}");
            None
        }
    }
}
