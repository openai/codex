use crate::common::SafetyBufferingTreatment;
use http::HeaderMap;

pub(crate) const X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER: &str = "x-codex-safety-buffering-enabled";
pub(crate) const X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER: &str =
    "x-codex-safety-buffering-faster-model";

pub(crate) fn treatment_from_headers(headers: &HeaderMap) -> Option<SafetyBufferingTreatment> {
    let show_buffering_ui = headers
        .get(X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER)
        .and_then(|value| value.to_str().ok())?
        .eq_ignore_ascii_case("true");
    let faster_model = if show_buffering_ui {
        headers
            .get(X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    } else {
        None
    };

    Some(SafetyBufferingTreatment {
        show_buffering_ui,
        faster_model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;

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
            Some(SafetyBufferingTreatment {
                show_buffering_ui: true,
                faster_model: Some("faster-model".to_string()),
            })
        );
    }
}
