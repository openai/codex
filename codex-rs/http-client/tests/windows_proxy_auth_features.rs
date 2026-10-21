#![cfg(windows)]

#[test]
fn curl_supports_windows_proxy_authentication() {
    let version = curl::Version::get();
    assert!(
        version.feature_ntlm() && version.feature_spnego() && version.feature_sspi(),
        "{version:?}"
    );
}
