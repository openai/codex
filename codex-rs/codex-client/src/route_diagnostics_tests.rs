use super::*;
use pretty_assertions::assert_eq;

#[test]
fn proxy_endpoint_redacts_credentials_host_path_and_query() {
    let endpoint = RedactedProxyEndpoint::parse(
        "http://user:secret@proxy.internal.example:8080/pac?token=secret",
    );

    assert_eq!(endpoint.to_string(), "http://<redacted-host>:8080");
    assert!(!format!("{endpoint:?}").contains("secret"));
    assert!(!format!("{endpoint}").contains("proxy.internal"));
}

#[test]
fn invalid_proxy_url_is_not_echoed() {
    let endpoint = RedactedProxyEndpoint::parse("not a url with password=secret");

    assert_eq!(endpoint.to_string(), "<invalid-proxy-url>");
}
