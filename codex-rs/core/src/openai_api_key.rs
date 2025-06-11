use std::env;
use std::sync::LazyLock;
use std::sync::RwLock;

pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";

static OPENAI_API_KEY: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| {
    let val = env::var(OPENAI_API_KEY_ENV_VAR)
        .ok()
        .and_then(|s| if s.is_empty() { None } else { Some(s) });
    RwLock::new(val)
});

pub fn get_openai_api_key() -> Option<String> {
    #![allow(clippy::unwrap_used)]
    OPENAI_API_KEY.read().unwrap().clone()
}

pub fn set_openai_api_key(value: String) {
    #![allow(clippy::unwrap_used)]
    if !value.is_empty() {
        *OPENAI_API_KEY.write().unwrap() = Some(value);
    }
}
