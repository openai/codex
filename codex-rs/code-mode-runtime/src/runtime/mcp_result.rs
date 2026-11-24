//! MCP call results receive a private marker on the returned V8 object.
//! Only `text()` consults it. Copies and JSON round trips produce ordinary unmarked objects.

use serde::Deserialize;
use serde::Serialize;
use serde_json::value::RawValue;
use serde_with::Map;
use serde_with::serde_as;

fn marker<'s>(scope: &mut v8::PinScope<'s, '_>) -> v8::Local<'s, v8::Private> {
    let name = v8::String::new(scope, "codex.mcp_result");
    v8::Private::for_api(scope, name)
}

pub(super) fn mark(scope: &mut v8::PinScope<'_, '_>, value: v8::Local<'_, v8::Value>) {
    if let Ok(object) = v8::Local::<v8::Object>::try_from(value) {
        let key = marker(scope);
        let marked = v8::Local::<v8::Value>::from(v8::Boolean::new(scope, true));
        object.set_private(scope, key, marked);
    }
}

/// Check before serialization, since a `toJSON` hook can delete itself and return a replacement.
/// Look for the property without invoking getters or proxy traps.
pub(super) fn can_omit_fallback(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> bool {
    let Ok(mut object) = v8::Local::<v8::Object>::try_from(value) else {
        return false;
    };
    let key = marker(scope);
    if object.has_private(scope, key) != Some(true) {
        return false;
    }
    let Some(to_json) = v8::String::new(scope, "toJSON") else {
        return false;
    };
    let to_json = v8::Local::<v8::Name>::from(to_json);
    loop {
        if object.is_proxy() || object.has_own_property(scope, to_json) != Some(false) {
            return false;
        }
        let Some(prototype) = object.get_prototype(scope) else {
            return false;
        };
        let Ok(parent) = v8::Local::<v8::Object>::try_from(prototype) else {
            return prototype.is_null();
        };
        object = parent;
    }
}

#[serde_as]
#[derive(Deserialize, Serialize)]
struct Object<'a>(
    #[serde(borrow)]
    #[serde_as(as = "Map<_, _>")]
    Vec<(String, &'a RawValue)>,
);

impl<'a> Object<'a> {
    fn field(&self, name: &str) -> Option<&'a RawValue> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| *value)
    }
}

/// Inspect a single V8 serialization, so getters and toJSON run exactly once.
/// Raw fields preserve the order and spelling of the rest of the envelope.
pub(super) fn without_fallback(serialized: &str) -> Option<String> {
    let mut result: Object<'_> = serde_json::from_str(serialized).ok()?;
    if result
        .field("isError")
        .is_some_and(|value| value.get() != "false")
    {
        return None;
    }
    let structured = result.field("structuredContent")?;
    let [block]: [Object; 1] = serde_json::from_str(result.field("content")?.get()).ok()?;
    if structured.get() == "null" || block.0.len() != 2 || block.field("type")?.get() != r#""text""# {
        return None;
    }
    let text: String = serde_json::from_str(block.field("text")?.get()).ok()?;
    let fallback: &RawValue = serde_json::from_str(&text).ok()?;
    if !super::json_equality::equivalent(structured, fallback) {
        return None;
    }
    // Release the decoded copy before allocating the new output.
    drop(text);
    result.0.retain(|(key, _)| key != "content");
    serde_json::to_string(&result).ok()
}

#[cfg(test)]
#[path = "mcp_result_tests.rs"]
mod tests;
