use serde_json::Value as JsonValue;

use crate::session_runtime::OutputItem;
use codex_code_mode_protocol::DEFAULT_IMAGE_DETAIL;
use codex_code_mode_protocol::ImageDetail;
use codex_protocol::models::AudioReference;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageReference;

use super::RuntimeState;
use super::audio::wav_duration_seconds;

const CONTENT_HELPER_EXPECTS_MESSAGE: &str = "content expects one MCP ContentBlock";
const IMAGE_HELPER_EXPECTS_MESSAGE: &str = "image expects a non-empty image URL string, an object with image_url and optional detail, or a raw MCP image block";
const AUDIO_HELPER_EXPECTS_MESSAGE: &str = "audio expects a non-empty audio URL string, an object with audio_url, or a raw MCP audio block";
const REMOTE_IMAGE_URL_ERROR: &str = "Tool call failed: remote image URLs are not supported in tool outputs. Pass a base64 data URI instead";
const INVALID_IMAGE_URL_ERROR: &str =
    "Tool call failed: invalid image output. Pass a base64 data URI instead";
const INVALID_AUDIO_URL_ERROR: &str =
    "Tool call failed: invalid audio output. Pass a base64 data URI instead";

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

    if let Some(text) = stringify_json_value(scope, value)? {
        return Ok(text);
    }
    let tc = std::pin::pin!(v8::TryCatch::new(scope));
    Ok(value.to_rust_string_lossy(&tc.init()))
}

pub(super) fn normalize_output_content(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<OutputItem, String> {
    let block = v8_value_to_json(scope, value)?
        .filter(JsonValue::is_object)
        .ok_or_else(|| CONTENT_HELPER_EXPECTS_MESSAGE.to_string())?;
    mcp_output_item(&block)
}

fn mcp_output_item(block: &JsonValue) -> Result<OutputItem, String> {
    let item = FunctionCallOutputContentItem::from_mcp_content_block(block)
        .map_err(|_| CONTENT_HELPER_EXPECTS_MESSAGE.to_string())?;
    match item {
        FunctionCallOutputContentItem::InputText { text } => Ok(OutputItem::InputText { text }),
        FunctionCallOutputContentItem::InputImage {
            image: ImageReference::Inline { image_url },
            detail,
        } => Ok(OutputItem::InputImage { image_url, detail }),
        FunctionCallOutputContentItem::InputAudio {
            audio: AudioReference::Inline { audio_url },
        } => Ok(output_audio_item(audio_url)),
        FunctionCallOutputContentItem::InputAudio {
            audio: AudioReference::File { .. },
        } => Err("content expects inline MCP audio data".to_string()),
        FunctionCallOutputContentItem::EncryptedContent { .. } => {
            Err("content cannot emit encrypted MCP text".to_string())
        }
        FunctionCallOutputContentItem::InputImage {
            image: ImageReference::File { .. },
            ..
        } => Err("content expects inline MCP image data".to_string()),
    }
}

pub(super) fn normalize_output_image(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
    detail_override: Option<v8::Local<'_, v8::Value>>,
) -> Result<OutputItem, ()> {
    let result = (|| -> Result<OutputItem, String> {
        let mut explicit_detail = parse_image_detail_value(scope, detail_override)?;
        let (image_url, mcp_detail) = if value.is_string() {
            (value.to_rust_string_lossy(scope), None)
        } else if value.is_object() && !value.is_array() {
            let object = v8::Local::<v8::Object>::try_from(value)
                .map_err(|_| IMAGE_HELPER_EXPECTS_MESSAGE.to_string())?;
            if let Some((image_url, detail)) = parse_non_mcp_output_image(scope, object)? {
                explicit_detail = explicit_detail.or(detail);
                (image_url, None)
            } else {
                let OutputItem::InputImage { image_url, detail } =
                    parse_mcp_media_block(scope, value, "image", IMAGE_HELPER_EXPECTS_MESSAGE)?
                else {
                    return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
                };
                (image_url, detail)
            }
        } else {
            return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
        };

        if image_url.is_empty() {
            return Err(IMAGE_HELPER_EXPECTS_MESSAGE.to_string());
        }
        let Some((scheme, _)) = image_url.split_once(':') else {
            return Err(INVALID_IMAGE_URL_ERROR.to_string());
        };
        if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
            return Err(REMOTE_IMAGE_URL_ERROR.to_string());
        }
        if !scheme.eq_ignore_ascii_case("data") {
            return Err(INVALID_IMAGE_URL_ERROR.to_string());
        }

        let detail = if let Some(mut detail) = explicit_detail {
            detail.make_ascii_lowercase();
            serde_json::from_value::<ImageDetail>(JsonValue::String(detail))
                .map_err(|_| "image detail must be one of: auto, low, high, original".to_string())?
        } else {
            mcp_detail.unwrap_or(DEFAULT_IMAGE_DETAIL)
        };

        Ok(OutputItem::InputImage {
            image_url,
            detail: Some(detail),
        })
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

fn parse_mcp_media_block(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
    expected_type: &str,
    expects_message: &str,
) -> Result<OutputItem, String> {
    let Some(JsonValue::Object(mut block)) = v8_value_to_json(scope, value)? else {
        return Err(expects_message.to_string());
    };
    let Some(item_type) = block.get("type").and_then(JsonValue::as_str) else {
        return Err(expects_message.to_string());
    };
    if item_type != expected_type {
        return Err(format!(
            "{expected_type} only accepts MCP {expected_type} blocks, got \"{item_type}\""
        ));
    }
    if block
        .get("data")
        .and_then(JsonValue::as_str)
        .is_none_or(str::is_empty)
    {
        return Err(format!("{expected_type} expected MCP {expected_type} data"));
    }
    // image() and audio() historically prefer mimeType over its alias and treat
    // an empty or non-string MIME type as absent.
    if let Some(alias) = block.remove("mime_type") {
        block.entry("mimeType").or_insert(alias);
    }
    if block
        .get("mimeType")
        .is_some_and(|mime| mime.as_str().is_none_or(str::is_empty))
    {
        block.remove("mimeType");
    }
    mcp_output_item(&JsonValue::Object(block))
}

fn parse_image_detail_value(
    scope: &mut v8::PinScope<'_, '_>,
    value: Option<v8::Local<'_, v8::Value>>,
) -> Result<Option<String>, String> {
    match value {
        Some(value) if value.is_string() => Ok(Some(value.to_rust_string_lossy(scope))),
        Some(value) if value.is_null() || value.is_undefined() => Ok(None),
        Some(_) => Err("image detail must be a string when provided".to_string()),
        None => Ok(None),
    }
}

pub(super) fn normalize_output_audio(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<OutputItem, ()> {
    let result = (|| -> Result<OutputItem, String> {
        let audio_url = if value.is_string() {
            value.to_rust_string_lossy(scope)
        } else if value.is_object() && !value.is_array() {
            let object = v8::Local::<v8::Object>::try_from(value)
                .map_err(|_| AUDIO_HELPER_EXPECTS_MESSAGE.to_string())?;
            if let Some(audio_url) = parse_non_mcp_output_audio(scope, object)? {
                audio_url
            } else {
                return parse_mcp_media_block(scope, value, "audio", AUDIO_HELPER_EXPECTS_MESSAGE);
            }
        } else {
            return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
        };

        if audio_url.is_empty() {
            return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
        }
        let Some((scheme, _)) = audio_url.split_once(':') else {
            return Err(INVALID_AUDIO_URL_ERROR.to_string());
        };
        if !scheme.eq_ignore_ascii_case("data") {
            return Err(INVALID_AUDIO_URL_ERROR.to_string());
        }

        Ok(output_audio_item(audio_url))
    })();

    match result {
        Ok(item) => Ok(item),
        Err(error_text) => {
            throw_type_error(scope, &error_text);
            Err(())
        }
    }
}

fn output_audio_item(audio_url: String) -> OutputItem {
    // Tiny tool-generated clips cannot be encoded reliably by audio models.
    if wav_duration_seconds(&audio_url).is_some_and(|duration| duration < 0.025) {
        OutputItem::InputText {
            text: "Audio output omitted because the clip is shorter than 25 ms; use a longer clip."
                .to_string(),
        }
    } else {
        OutputItem::InputAudio { audio_url }
    }
}

fn parse_non_mcp_output_audio(
    scope: &mut v8::PinScope<'_, '_>,
    object: v8::Local<'_, v8::Object>,
) -> Result<Option<String>, String> {
    let audio_url_key = v8::String::new(scope, "audio_url")
        .ok_or_else(|| "failed to allocate audio helper keys".to_string())?;
    let Some(audio_url) = object.get(scope, audio_url_key.into()) else {
        return Ok(None);
    };
    if audio_url.is_undefined() {
        return Ok(None);
    }
    if !audio_url.is_string() {
        return Err(AUDIO_HELPER_EXPECTS_MESSAGE.to_string());
    }
    Ok(Some(audio_url.to_rust_string_lossy(scope)))
}

fn stringify_json_value(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<Option<String>, String> {
    let tc = std::pin::pin!(v8::TryCatch::new(scope));
    let mut tc = tc.init();
    if let Some(stringified) = v8::json::stringify(&tc, value) {
        return Ok(Some(stringified.to_rust_string_lossy(&tc)));
    }
    if tc.has_caught() {
        return Err(tc
            .exception()
            .map(|exception| value_to_error_text(&mut tc, exception))
            .unwrap_or_else(|| "unknown code mode exception".to_string()));
    }
    Ok(None)
}

pub(super) fn v8_value_to_json(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<Option<JsonValue>, String> {
    // Reentering V8 during termination can replace it with a catchable exception.
    if scope.is_execution_terminating() {
        return Err("JavaScript execution terminated".to_string());
    }
    // V8 stringifies undefined as the non-JSON text "undefined".
    if value.is_undefined() {
        return Ok(None);
    }

    let Some(json) = stringify_json_value(scope, value)? else {
        return Ok(None);
    };
    // V8 emits compact JSON with these ASCII key names unescaped. The object/comma
    // prefix and colon distinguish keys from string contents. Reject these before
    // Serde can reinterpret them as values and bypass its normal recursion limit.
    // TODO(cconger): Remove this guard once our resolved serde_json dependency
    // includes the upstream private-key provenance fix. Retain regression coverage
    // for literal keys, depth limits, and session recovery.
    // https://github.com/cconger/json/commit/63f6f14053ef3a173013d7e5ee5ac74140736929
    if [
        r#"{"$serde_json::private::RawValue":"#,
        r#","$serde_json::private::RawValue":"#,
        r#"{"$serde_json::private::Number":"#,
        r#","$serde_json::private::Number":"#,
    ]
    .into_iter()
    .any(|key| json.contains(key))
    {
        return Err("failed to serialize JavaScript value: reserved JSON object key".to_string());
    }
    serde_json::from_str(&json)
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
    // The stack getter can call exit() and V8 can clear its termination before
    // returning here. Never enter a user-defined toString() after that.
    if scope
        .get_slot::<RuntimeState>()
        .is_some_and(|state| state.exit_requested)
    {
        // send_scope_result reports this as successful exit and preserves prior stores.
        return String::new();
    }
    value.to_rust_string_lossy(scope)
}

pub(super) fn throw_type_error(scope: &mut v8::PinScope<'_, '_>, message: &str) {
    // Preserve V8's uncatchable termination instead of replacing it with a JS error.
    if scope.is_execution_terminating() {
        return;
    }
    if let Some(message) = v8::String::new(scope, message) {
        scope.throw_exception(message.into());
    }
}
