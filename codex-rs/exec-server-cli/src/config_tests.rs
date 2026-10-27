#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn exec_server_remote_auth_accepts_api_key_auth() {
    let auth = CodexAuth::from_api_key("sk-test");

    assert!(is_supported_exec_server_remote_auth(&auth));
}

#[test]
fn exec_server_remote_api_key_auth_accepts_https_openai_domains() {
    for base_url in [
        "https://openai.com/api",
        "https://service.openai.com/api",
        "https://openai.org/api",
        "https://service.openai.org/api",
    ] {
        assert!(validate_api_key_remote_host(base_url).is_ok());
    }
}

#[test]
fn exec_server_remote_api_key_auth_accepts_http_loopback() {
    for base_url in [
        "http://localhost:8098/api",
        "http://127.0.0.1:8098/api",
        "http://[::1]:8098/api",
    ] {
        assert!(validate_api_key_remote_host(base_url).is_ok());
    }
}

#[test]
fn exec_server_remote_api_key_auth_rejects_http_openai_domain() {
    for base_url in [
        "http://service.openai.com/api",
        "http://service.openai.org/api",
    ] {
        let error = validate_api_key_remote_host(base_url)
            .expect_err("reject plaintext OpenAI destination");

        assert_eq!(
            error.to_string(),
            "remote exec-server API-key authentication is restricted to HTTPS openai.com and openai.org hosts and subdomains or loopback hosts"
        );
    }
}

#[test]
fn exec_server_remote_api_key_auth_rejects_suffix_spoof() {
    let error = validate_api_key_remote_host("https://service.openai.org.evil.example/api")
        .expect_err("reject suffix spoof");

    assert_eq!(
        error.to_string(),
        "remote exec-server API-key authentication is restricted to HTTPS openai.com and openai.org hosts and subdomains or loopback hosts"
    );
}
