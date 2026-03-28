use serde_json::Value as JsonValue;

use crate::response::FunctionCallOutputContentItem;
use crate::response::ImageDetail;

pub(super) fn serialize_output_text(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<String, String> {
    if value.is_undefined()
        || value.is_null()
        || value.is_boolean()
        || value.is_number()
        || value.is_big_int()
        || value.is_string()
    {
        return Ok(value.to_rust_string_lossy(scope));
    }

    let tc = std::pin::pin!(v8::TryCatch::new(scope));
    let mut tc = tc.init();
    if let Some(stringified) = v8::json::stringify(&tc, value) {
        return Ok(stringified.to_rust_string_lossy(&tc));
    }
    if tc.has_caught() {
        return Err(tc
            .exception()
            .map(|exception| value_to_error_text(&mut tc, exception))
            .unwrap_or_else(|| "unknown code mode exception".to_string()));
    }
    Ok(value.to_rust_string_lossy(&tc))
}

pub(super) fn normalize_output_image(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<FunctionCallOutputContentItem, String> {
    (|| -> Result<FunctionCallOutputContentItem, String> {
        let (image_url, detail) = if value.is_string() {
            (value.to_rust_string_lossy(scope), None)
        } else if value.is_object() && !value.is_array() {
            let object = v8::Local::<v8::Object>::try_from(value).map_err(|_| {
                "image expects a non-empty image URL string or an object with image_url and optional detail".to_string()
            })?;
            let image_url_key = v8::String::new(scope, "image_url")
                .ok_or_else(|| "failed to allocate image helper keys".to_string())?;
            let detail_key = v8::String::new(scope, "detail")
                .ok_or_else(|| "failed to allocate image helper keys".to_string())?;
            let image_url = object
                .get(scope, image_url_key.into())
                .filter(|value| value.is_string())
                .map(|value| value.to_rust_string_lossy(scope))
                .ok_or_else(|| {
                    "image expects a non-empty image URL string or an object with image_url and optional detail"
                        .to_string()
                })?;
            let detail = match object.get(scope, detail_key.into()) {
                Some(value) if value.is_string() => Some(value.to_rust_string_lossy(scope)),
                Some(value) if value.is_null() || value.is_undefined() => None,
                Some(_) => return Err("image detail must be a string when provided".to_string()),
                None => None,
            };
            (image_url, detail)
        } else {
            return Err(
                "image expects a non-empty image URL string or an object with image_url and optional detail"
                    .to_string(),
            );
        };

        if image_url.is_empty() {
            return Err(
                "image expects a non-empty image URL string or an object with image_url and optional detail"
                    .to_string(),
            );
        }
        let lower = image_url.to_ascii_lowercase();
        if !(lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("data:"))
        {
            return Err("image expects an http(s) or data URL".to_string());
        }

        let detail = match detail {
            Some(detail) => {
                let normalized = detail.to_ascii_lowercase();
                Some(match normalized.as_str() {
                    "auto" => ImageDetail::Auto,
                    "low" => ImageDetail::Low,
                    "high" => ImageDetail::High,
                    "original" => ImageDetail::Original,
                    _ => {
                        return Err(
                            "image detail must be one of: auto, low, high, original".to_string()
                        );
                    }
                })
            }
            None => None,
        };

        Ok(FunctionCallOutputContentItem::InputImage { image_url, detail })
    })()
}

pub(super) fn v8_value_to_json(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<Option<JsonValue>, String> {
    let tc = std::pin::pin!(v8::TryCatch::new(scope));
    let mut tc = tc.init();
    let Some(stringified) = v8::json::stringify(&tc, value) else {
        if tc.has_caught() {
            return Err(tc
                .exception()
                .map(|exception| value_to_error_text(&mut tc, exception))
                .unwrap_or_else(|| "unknown code mode exception".to_string()));
        }
        return Ok(None);
    };
    serde_json::from_str(&stringified.to_rust_string_lossy(&tc))
        .map(Some)
        .map_err(|err| format!("failed to serialize JavaScript value: {err}"))
}

pub(super) fn json_to_v8<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: &JsonValue,
) -> Option<v8::Local<'s, v8::Value>> {
    let json = serde_json::to_string(value).ok()?;
    let json = v8::String::new(scope, &json)?;
    v8::json::parse(scope, json)
}

pub(super) fn value_to_error_text(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> String {
    if value.is_object()
        && let Ok(object) = v8::Local::<v8::Object>::try_from(value)
        && let Some(key) = v8::String::new(scope, "stack")
        && let Some(stack) = object.get(scope, key.into())
        && stack.is_string()
    {
        return stack.to_rust_string_lossy(scope);
    }
    value.to_rust_string_lossy(scope)
}

pub(super) fn throw_type_error(scope: &mut v8::PinScope<'_, '_>, message: &str) {
    if let Some(message) = v8::String::new(scope, message) {
        scope.throw_exception(message.into());
    }
}
