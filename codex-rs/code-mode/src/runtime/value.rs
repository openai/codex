use serde_json::Value as JsonValue;

use crate::response::DEFAULT_IMAGE_DETAIL;
use crate::response::FunctionCallOutputContentItem;
use crate::response::ImageDetail;
use crate::response::InputAudio;

const IMAGE_HELPER_EXPECTS_MESSAGE: &str = "image expects a non-empty image URL string, an object with image_url and optional detail, or a raw MCP image block";
const AUDIO_HELPER_EXPECTS_MESSAGE: &str = "audio expects an object with non-empty data and format/mimeType/mime_type, or a raw MCP audio block";
const CODEX_IMAGE_DETAIL_META_KEY: &str = "codex/imageDetail";

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
    detail_override: Option<String>,
) -> Result<FunctionCallOutputContentItem, ()> {
    let result = (|| -> Result<FunctionCallOutputContentItem, String> {
        let (image_url, detail) = if value.is_string() {
            (value.to_rust_string_lossy(scope), None)
        } else if value.is_object() && !value.is_array() {
            let object = v8::Local::<v8::Object>::try_from(value)
                .map_err(|_| IMAGE_HELPER_EXPECTS_MESSAGE.to_string())?;
            if let Some(image) = parse_non_mcp_output_image(scope, object)? {
                image
            } else {
                parse_mcp_output_image(scope, value)?
            }
        } else {
            return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
        };

        if image_url.is_empty() {
            return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
        }
        let lower = image_url.to_ascii_lowercase();
        if !(lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("data:"))
        {
            return Err("image expects an http(s) or data URL".to_string());
        }

        let detail = detail_override.or(detail);
        let detail = match detail {
            Some(detail) => {
                let normalized = detail.to_ascii_lowercase();
                Some(match normalized.as_str() {
                    "high" => ImageDetail::High,
                    "original" => ImageDetail::Original,
                    _ => {
                        return Err("image detail must be one of: high, original".to_string());
                    }
                })
            }
            None => Some(DEFAULT_IMAGE_DETAIL),
        };

        Ok(FunctionCallOutputContentItem::InputImage { image_url, detail })
    })();

    match result {
        Ok(item) => Ok(item),
        Err(error_text) => {
            throw_type_error(scope, &error_text);
            Err(())
        }
    }
}

pub(super) fn normalize_output_audio(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<FunctionCallOutputContentItem, ()> {
    let result = (|| -> Result<FunctionCallOutputContentItem, String> {
        if !value.is_object() || value.is_array() {
            return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
        }

        let object = v8::Local::<v8::Object>::try_from(value)
            .map_err(|_| AUDIO_HELPER_EXPECTS_MESSAGE.to_string())?;
        let input_audio = if let Some(audio) = parse_non_mcp_output_audio(scope, object)? {
            audio
        } else {
            parse_mcp_output_audio(scope, value)?
        };

        Ok(FunctionCallOutputContentItem::InputAudio { input_audio })
    })();

    match result {
        Ok(item) => Ok(item),
        Err(error_text) => {
            throw_type_error(scope, &error_text);
            Err(())
        }
    }
}

fn parse_non_mcp_output_image(
    scope: &mut v8::PinScope<'_, '_>,
    object: v8::Local<'_, v8::Object>,
) -> Result<Option<(String, Option<String>)>, String> {
    let image_url_key = v8::String::new(scope, "image_url")
        .ok_or_else(|| "failed to allocate image helper keys".to_string())?;
    let Some(image_url) = object.get(scope, image_url_key.into()) else {
        return Ok(None);
    };
    if image_url.is_undefined() {
        return Ok(None);
    }
    if !image_url.is_string() {
        return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
    }
    let detail_key = v8::String::new(scope, "detail")
        .ok_or_else(|| "failed to allocate image helper keys".to_string())?;
    let detail = parse_image_detail_value(scope, object.get(scope, detail_key.into()))?;
    Ok(Some((image_url.to_rust_string_lossy(scope), detail)))
}

fn parse_mcp_output_image(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<(String, Option<String>), String> {
    let Some(result) = v8_value_to_json(scope, value)? else {
        return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
    };
    let JsonValue::Object(result) = result else {
        return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
    };
    let Some(item_type) = result.get("type").and_then(JsonValue::as_str) else {
        return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
    };
    if item_type != "image" {
        return Err(format!(
            "image only accepts MCP image blocks, got \"{item_type}\""
        ));
    }
    let data = result
        .get("data")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "image expected MCP image data".to_string())?;
    if data.is_empty() {
        return Err("image expected MCP image data".to_string());
    }

    let image_url = if data.to_ascii_lowercase().starts_with("data:") {
        data.to_string()
    } else {
        let mime_type = result
            .get("mimeType")
            .or_else(|| result.get("mime_type"))
            .and_then(JsonValue::as_str)
            .filter(|mime_type| !mime_type.is_empty())
            .unwrap_or("application/octet-stream");
        format!("data:{mime_type};base64,{data}")
    };
    let detail = result
        .get("_meta")
        .and_then(JsonValue::as_object)
        .and_then(|meta| meta.get(CODEX_IMAGE_DETAIL_META_KEY))
        .and_then(JsonValue::as_str)
        .filter(|detail| matches!(*detail, "high" | "original"))
        .map(str::to_string);
    Ok((image_url, detail))
}

fn parse_non_mcp_output_audio(
    scope: &mut v8::PinScope<'_, '_>,
    object: v8::Local<'_, v8::Object>,
) -> Result<Option<InputAudio>, String> {
    let data_key = v8::String::new(scope, "data")
        .ok_or_else(|| "failed to allocate audio helper keys".to_string())?;
    let Some(data) = object.get(scope, data_key.into()) else {
        return Ok(None);
    };
    if data.is_undefined() {
        return Ok(None);
    }
    if !data.is_string() {
        return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
    }
    let data = data.to_rust_string_lossy(scope);
    let format = optional_string_property(scope, object, "format")?;
    let mime_type = optional_string_property(scope, object, "mimeType")?
        .or(optional_string_property(scope, object, "mime_type")?);
    let Some(input_audio) = codex_protocol::models::input_audio_from_data(
        &data,
        format.as_deref(),
        mime_type.as_deref(),
    ) else {
        return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
    };
    Ok(Some(InputAudio {
        data: input_audio.data,
        format: input_audio.format,
    }))
}

fn parse_mcp_output_audio(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<InputAudio, String> {
    let Some(result) = v8_value_to_json(scope, value)? else {
        return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
    };
    let JsonValue::Object(result) = result else {
        return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
    };
    let Some(item_type) = result.get("type").and_then(JsonValue::as_str) else {
        return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
    };
    if item_type != "audio" {
        return Err(format!(
            "audio only accepts MCP audio blocks, got \"{item_type}\""
        ));
    }
    let data = result
        .get("data")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "audio expected MCP audio data".to_string())?;
    let mime_type = result
        .get("mimeType")
        .or_else(|| result.get("mime_type"))
        .and_then(JsonValue::as_str);
    let Some(input_audio) =
        codex_protocol::models::input_audio_from_data(data, /*format*/ None, mime_type)
    else {
        return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
    };
    Ok(InputAudio {
        data: input_audio.data,
        format: input_audio.format,
    })
}

fn optional_string_property(
    scope: &mut v8::PinScope<'_, '_>,
    object: v8::Local<'_, v8::Object>,
    name: &str,
) -> Result<Option<String>, String> {
    let key = v8::String::new(scope, name)
        .ok_or_else(|| "failed to allocate audio helper keys".to_string())?;
    match object.get(scope, key.into()) {
        Some(value) if value.is_string() => Ok(Some(value.to_rust_string_lossy(scope))),
        Some(value) if value.is_null() || value.is_undefined() => Ok(None),
        Some(_) => Err(format!("{name} must be a string when provided")),
        None => Ok(None),
    }
}

fn parse_image_detail_value<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: Option<v8::Local<'s, v8::Value>>,
) -> Result<Option<String>, String> {
    match value {
        Some(value) if value.is_string() => Ok(Some(value.to_rust_string_lossy(scope))),
        Some(value) if value.is_null() || value.is_undefined() => Ok(None),
        Some(_) => Err("image detail must be a string when provided".to_string()),
        None => Ok(None),
    }
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
