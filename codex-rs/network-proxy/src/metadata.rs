use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rama_http::HeaderValue;

pub const NETWORK_ATTEMPT_USERNAME_PREFIX: &str = "codex-net-attempt-";

pub fn proxy_username_for_attempt_id(attempt_id: &str) -> String {
    format!("{NETWORK_ATTEMPT_USERNAME_PREFIX}{attempt_id}")
}

pub fn attempt_id_from_proxy_authorization(header: Option<&HeaderValue>) -> Option<String> {
    let header = header?;
    let raw = header.to_str().ok()?;
    let encoded = raw.strip_prefix("Basic ")?;
    let decoded = STANDARD.decode(encoded.trim()).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let username = decoded
        .split_once(':')
        .map(|(user, _)| user)
        .unwrap_or(decoded.as_str());
    let attempt_id = username.strip_prefix(NETWORK_ATTEMPT_USERNAME_PREFIX)?;
    if attempt_id.is_empty() {
        None
    } else {
        Some(attempt_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;

    #[test]
    fn parses_attempt_id_from_proxy_authorization_header() {
        let encoded = STANDARD.encode(format!("{NETWORK_ATTEMPT_USERNAME_PREFIX}abc123:"));
        let header = HeaderValue::from_str(&format!("Basic {encoded}")).unwrap();
        assert_eq!(
            attempt_id_from_proxy_authorization(Some(&header)),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn ignores_non_attempt_proxy_authorization_header() {
        let encoded = STANDARD.encode("normal-user:password");
        let header = HeaderValue::from_str(&format!("Basic {encoded}")).unwrap();
        assert_eq!(attempt_id_from_proxy_authorization(Some(&header)), None);
    }
}
