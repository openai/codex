use super::*;
use crate::PROXY_ACTIVE_ENV_KEY;
use pretty_assertions::assert_eq;

#[test]
fn strips_managed_proxy_env() {
    let mut env = HashMap::from([
        (PROXY_ACTIVE_ENV_KEY.to_string(), "1".to_string()),
        (
            "HTTPS_PROXY".to_string(),
            "http://127.0.0.1:1234".to_string(),
        ),
        ("CUSTOM_ENV".to_string(), "kept".to_string()),
    ]);

    strip_managed_proxy_env(&mut env);

    assert_eq!(
        env,
        HashMap::from([("CUSTOM_ENV".to_string(), "kept".to_string())])
    );
}

#[test]
fn preserves_unmanaged_ca_env() {
    let mut env = HashMap::from([(
        "SSL_CERT_FILE".to_string(),
        "/tmp/user-ca-bundle.pem".to_string(),
    )]);

    strip_managed_proxy_env(&mut env);

    assert_eq!(
        env,
        HashMap::from([(
            "SSL_CERT_FILE".to_string(),
            "/tmp/user-ca-bundle.pem".to_string(),
        )])
    );
}
