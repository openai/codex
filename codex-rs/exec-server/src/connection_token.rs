use axum::http::HeaderValue;

const CONNECTION_TOKEN_ENV_VAR: &str = "CODEX_EXEC_SERVER_CONNECTION_TOKEN";

pub(crate) fn connection_token_from_env() -> Result<Option<HeaderValue>, String> {
    let token = match std::env::var(CONNECTION_TOKEN_ENV_VAR) {
        Ok(token) => token,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!("{CONNECTION_TOKEN_ENV_VAR} must be valid Unicode"));
        }
    };
    if token.is_empty() {
        return Err(format!("{CONNECTION_TOKEN_ENV_VAR} must not be empty"));
    }
    let mut header = HeaderValue::from_str(&format!("Bearer {token}"))
        .map_err(|_| format!("{CONNECTION_TOKEN_ENV_VAR} must be a valid HTTP header value"))?;
    header.set_sensitive(true);
    Ok(Some(header))
}
