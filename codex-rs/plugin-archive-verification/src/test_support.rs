//! Builds key requests for the hermetic Plugin Service fixtures.

use crate::ArchiveVerificationFailure;
use codex_http_client::HttpClientBuilder;
use codex_http_client::RequestBuilder;
use http::HeaderMap;
use http::header::AUTHORIZATION;
use serde_json::Value;
use std::future::Ready;
use std::future::ready;

pub(super) fn fixture() -> Value {
    serde_json::from_str(include_str!("../testdata/plugin_archive_signing_v1.json"))
        .expect("valid signing vector")
}

pub(super) fn auth_headers(token: &str, account: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
    headers.insert("chatgpt-account-id", account.parse().unwrap());
    headers
}

pub(super) fn key_request(
    headers: HeaderMap,
) -> impl FnOnce(String) -> Ready<Result<(RequestBuilder, HeaderMap), ArchiveVerificationFailure>> {
    move |url| {
        let client = HttpClientBuilder::new().build_direct().unwrap();
        ready(Ok((client.get(url), headers)))
    }
}
