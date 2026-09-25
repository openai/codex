//! Converts headers in Responses event payloads to HTTP headers.
//! Invalid names and unsupported or invalid values are ignored.

use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use serde_json::Value;
use serde_json::map::Map as JsonMap;

pub(crate) fn json_headers_to_http_headers(headers: &JsonMap<String, Value>) -> HeaderMap {
    let mut mapped = HeaderMap::new();
    for (name, value) in headers {
        let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Some(header_value) = json_header_value(value) else {
            continue;
        };
        mapped.insert(header_name, header_value);
    }
    mapped
}

fn json_header_value(value: &Value) -> Option<HeaderValue> {
    let value = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => return None,
    };
    HeaderValue::from_str(&value).ok()
}
