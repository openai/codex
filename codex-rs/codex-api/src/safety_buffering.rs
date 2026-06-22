use crate::common::SafetyBufferingTreatment;
use http::HeaderMap;
use serde_json::Value;

pub(crate) const X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER: &str = "x-codex-safety-buffering-enabled";
pub(crate) const X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER: &str =
    "x-codex-safety-buffering-faster-model";

pub(crate) fn treatment_from_headers(headers: &HeaderMap) -> SafetyBufferingTreatment {
    let show_buffering_ui = headers
        .get(X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));
    let faster_model = if show_buffering_ui {
        headers
            .get(X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    } else {
        None
    };

    SafetyBufferingTreatment {
        show_buffering_ui,
        faster_model,
    }
}

pub(crate) fn treatment_from_json_headers(value: &Value) -> Option<SafetyBufferingTreatment> {
    let headers = value.as_object()?;
    let enabled = headers.iter().find_map(|(name, value)| {
        if name.eq_ignore_ascii_case(X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER) {
            json_value_as_string(value)
        } else {
            None
        }
    })?;
    let show_buffering_ui = enabled.eq_ignore_ascii_case("true");
    let faster_model = if show_buffering_ui {
        headers.iter().find_map(|(name, value)| {
            if name.eq_ignore_ascii_case(X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER) {
                json_value_as_string(value)
            } else {
                None
            }
        })
    } else {
        None
    };

    Some(SafetyBufferingTreatment {
        show_buffering_ui,
        faster_model,
    })
}

fn json_value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn reads_treatment_from_http_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER,
            HeaderValue::from_static("true"),
        );
        headers.insert(
            X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER,
            HeaderValue::from_static("faster-model"),
        );

        assert_eq!(
            treatment_from_headers(&headers),
            SafetyBufferingTreatment {
                show_buffering_ui: true,
                faster_model: Some("faster-model".to_string()),
            }
        );
    }

    #[test]
    fn reads_treatment_from_websocket_metadata_headers() {
        assert_eq!(
            treatment_from_json_headers(&json!({
                "x-codex-safety-buffering-enabled": "true",
                "x-codex-safety-buffering-faster-model": "faster-model"
            })),
            Some(SafetyBufferingTreatment {
                show_buffering_ui: true,
                faster_model: Some("faster-model".to_string()),
            })
        );
    }
}
