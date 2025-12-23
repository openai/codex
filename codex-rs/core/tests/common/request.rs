use http::header::CONTENT_ENCODING;
use serde::de::DeserializeOwned;
use wiremock::Match;

pub fn decoded_body_bytes(request: &wiremock::Request) -> Vec<u8> {
    if is_zstd_encoded(request) {
        zstd::decode_all(request.body.as_slice()).unwrap_or_else(|err| {
            panic!("failed to decode zstd-encoded request body: {err}");
        })
    } else {
        request.body.clone()
    }
}

pub fn decoded_body_string(request: &wiremock::Request) -> String {
    String::from_utf8_lossy(&decoded_body_bytes(request)).into_owned()
}

pub trait RequestBodyExt {
    fn json_body<T: DeserializeOwned>(&self) -> T;
    fn text_body(&self) -> String;
}

impl RequestBodyExt for wiremock::Request {
    fn json_body<T: DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&decoded_body_bytes(self)).unwrap_or_else(|err| {
            panic!("failed to decode request body as JSON: {err}");
        })
    }

    fn text_body(&self) -> String {
        decoded_body_string(self)
    }
}

pub fn body_contains(needle: impl Into<String>) -> impl Match {
    BodyContains {
        needle: needle.into(),
    }
}

struct BodyContains {
    needle: String,
}

impl Match for BodyContains {
    fn matches(&self, request: &wiremock::Request) -> bool {
        decoded_body_string(request).contains(self.needle.as_str())
    }
}

fn is_zstd_encoded(request: &wiremock::Request) -> bool {
    request
        .headers
        .get(CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case("zstd"))
        .unwrap_or(false)
}
