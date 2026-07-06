use crate::common::SafetyBufferingTreatment;
use http::HeaderMap;

pub(crate) const X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER: &str = "x-codex-safety-buffering-enabled";
pub(crate) const X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER: &str =
    "x-codex-safety-buffering-faster-model";

pub(crate) fn treatment_from_headers(headers: &HeaderMap) -> Option<SafetyBufferingTreatment> {
    if !headers.contains_key(X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER)
        && !headers.contains_key(X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER)
    {
        return None;
    }
    let faster_model = headers
        .get(X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    Some(SafetyBufferingTreatment { faster_model })
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
                faster_model: Some("faster-model".to_string()),
            })
        );
    }

    #[test]
    fn buffering_enabled_header_does_not_gate_the_faster_model_fallback() {
        let mut headers = HeaderMap::new();
        headers.insert(
            X_CODEX_SAFETY_BUFFERING_ENABLED_HEADER,
            HeaderValue::from_static("false"),
        );
        headers.insert(
            X_CODEX_SAFETY_BUFFERING_FASTER_MODEL_HEADER,
            HeaderValue::from_static("faster-model"),
        );

        assert_eq!(
            treatment_from_headers(&headers),
            Some(SafetyBufferingTreatment {
                faster_model: Some("faster-model".to_string()),
            })
        );
    }
}
