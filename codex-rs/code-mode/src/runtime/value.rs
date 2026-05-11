use serde_json::Value as JsonValue;

use crate::response::DEFAULT_IMAGE_DETAIL;
use crate::response::FunctionCallOutputContentItem;
use crate::response::ImageDetail;

const IMAGE_HELPER_EXPECTS_MESSAGE: &str = "image expects a non-empty image URL string, an object with image_url and optional detail, or a raw MCP image block";
const CODEX_IMAGE_DETAIL_META_KEY: &str = "codex/imageDetail";
const FORWARD_OUTPUT_HELPER_EXPECTS_MESSAGE: &str = "forward_output expects a direct tool output with `content` or `content_items`, a text/image content item, an image_url object, or a string output";

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

        validate_image_url(&image_url)?;

        let detail = detail_override.or(detail);
        let detail = match detail {
            Some(detail) => Some(parse_image_detail(&detail)?),
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

pub(super) fn normalize_forward_output_items(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<Vec<FunctionCallOutputContentItem>, String> {
    if value.is_string() {
        return Ok(vec![FunctionCallOutputContentItem::InputText {
            text: serialize_output_text(scope, value)?,
        }]);
    }

    let Some(json) = v8_value_to_json(scope, value)? else {
        return Err(FORWARD_OUTPUT_HELPER_EXPECTS_MESSAGE.to_string());
    };

    if let JsonValue::Object(object) = &json
        && let Some(items) = forward_output_items_from_tool_output_object(object)?
    {
        return Ok(items);
    }

    Err(FORWARD_OUTPUT_HELPER_EXPECTS_MESSAGE.to_string())
}

fn forward_output_items_from_tool_output_object(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<Option<Vec<FunctionCallOutputContentItem>>, String> {
    if let Some(content) = object.get("content") {
        let content = content
            .as_array()
            .ok_or_else(|| "output expected `content` to be an array".to_string())?;
        return content_items_from_array(content).map(Some);
    }

    if let Some(content_items) = object
        .get("content_items")
        .or_else(|| object.get("contentItems"))
    {
        let content_items = content_items
            .as_array()
            .ok_or_else(|| "output expected `content_items` to be an array".to_string())?;
        return content_items_from_array(content_items).map(Some);
    }

    if let Some(item) = forward_output_content_item_from_object(object)? {
        return Ok(Some(vec![item]));
    }

    if let Some(output) = object.get("output").and_then(JsonValue::as_str) {
        return Ok(Some(vec![FunctionCallOutputContentItem::InputText {
            text: output.to_string(),
        }]));
    }

    Ok(None)
}

fn content_items_from_array(
    values: &[JsonValue],
) -> Result<Vec<FunctionCallOutputContentItem>, String> {
    values
        .iter()
        .map(|value| {
            if let JsonValue::Object(object) = value {
                return forward_output_content_item_from_object(object)?
                    .ok_or_else(|| FORWARD_OUTPUT_HELPER_EXPECTS_MESSAGE.to_string());
            }
            Err("forward_output expected content entries to be text/image objects".to_string())
        })
        .collect()
}

fn forward_output_content_item_from_object(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<Option<FunctionCallOutputContentItem>, String> {
    if let Some(item_type) = object.get("type").and_then(JsonValue::as_str) {
        return match item_type {
            "text" | "input_text" | "inputText" => {
                Ok(Some(FunctionCallOutputContentItem::InputText {
                    text: required_string_field(object, "text", item_type)?.to_string(),
                }))
            }
            "image" => mcp_image_content_item_from_object(object).map(Some),
            "input_image" | "inputImage" => image_url_content_item_from_object(object).map(Some),
            _ => Err(format!(
                "forward_output only supports text and image content blocks, got `{item_type}`"
            )),
        };
    }

    if object.contains_key("image_url") || object.contains_key("imageUrl") {
        return image_url_content_item_from_object(object).map(Some);
    }

    Ok(None)
}

fn required_string_field<'a>(
    object: &'a serde_json::Map<String, JsonValue>,
    field: &str,
    item_type: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("output expected `{item_type}` content to include `{field}`"))
}

fn image_url_content_item_from_object(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<FunctionCallOutputContentItem, String> {
    let Some(image_url) = object.get("image_url").or_else(|| object.get("imageUrl")) else {
        return Err("output expected image content to include `image_url`".to_string());
    };
    let image_url = image_url
        .as_str()
        .ok_or_else(|| "output expected `image_url` to be a string".to_string())?;
    validate_image_url(image_url)?;
    let detail = json_image_detail_value(object.get("detail"))?.or(Some(DEFAULT_IMAGE_DETAIL));
    Ok(FunctionCallOutputContentItem::InputImage {
        image_url: image_url.to_string(),
        detail,
    })
}

fn mcp_image_content_item_from_object(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<FunctionCallOutputContentItem, String> {
    let data = required_string_field(object, "data", "image")?;
    if data.is_empty() {
        return Err("output expected MCP image data".to_string());
    }

    let image_url = if data.to_ascii_lowercase().starts_with("data:") {
        data.to_string()
    } else {
        let mime_type = object
            .get("mimeType")
            .or_else(|| object.get("mime_type"))
            .and_then(JsonValue::as_str)
            .filter(|mime_type| !mime_type.is_empty())
            .unwrap_or("application/octet-stream");
        format!("data:{mime_type};base64,{data}")
    };
    validate_image_url(&image_url)?;

    let detail = object
        .get("_meta")
        .and_then(JsonValue::as_object)
        .and_then(|meta| meta.get(CODEX_IMAGE_DETAIL_META_KEY))
        .and_then(JsonValue::as_str)
        .and_then(|detail| parse_image_detail(detail).ok())
        .or(Some(DEFAULT_IMAGE_DETAIL));

    Ok(FunctionCallOutputContentItem::InputImage { image_url, detail })
}

fn json_image_detail_value(value: Option<&JsonValue>) -> Result<Option<ImageDetail>, String> {
    match value {
        Some(JsonValue::String(detail)) => parse_image_detail(detail).map(Some),
        Some(JsonValue::Null) | None => Ok(None),
        Some(_) => Err("image detail must be a string when provided".to_string()),
    }
}

fn parse_image_detail(detail: &str) -> Result<ImageDetail, String> {
    let normalized = detail.to_ascii_lowercase();
    match normalized.as_str() {
        "auto" => Ok(ImageDetail::Auto),
        "low" => Ok(ImageDetail::Low),
        "high" => Ok(ImageDetail::High),
        "original" => Ok(ImageDetail::Original),
        _ => Err("image detail must be one of: auto, low, high, original".to_string()),
    }
}

fn validate_image_url(image_url: &str) -> Result<(), String> {
    if image_url.is_empty() {
        return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
    }
    let lower = image_url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("data:") {
        Ok(())
    } else {
        Err("image expects an http(s) or data URL".to_string())
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
        .filter(|detail| matches!(*detail, "auto" | "low" | "high" | "original"))
        .map(str::to_string);
    Ok((image_url, detail))
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
