use std::path::Path;
use std::sync::LazyLock;
use std::sync::RwLock;

use codex_login::TokenData;

static CHATGPT_TOKEN: LazyLock<RwLock<Option<TokenData>>> = LazyLock::new(|| RwLock::new(None));

pub fn get_chatgpt_token_data() -> Option<TokenData> {
    CHATGPT_TOKEN.read().ok()?.clone()
}

pub fn set_chatgpt_token_data(value: TokenData) {
    if let Ok(mut guard) = CHATGPT_TOKEN.write() {
        *guard = Some(value);
    }
}

/// Initialize the ChatGPT token from auth.json file
pub async fn init_chatgpt_token_from_auth(codex_home: &Path) -> std::io::Result<()> {
    let auth_json = codex_login::try_read_auth_json(codex_home).await?;
    set_chatgpt_token_data(auth_json.tokens.clone());
    Ok(())
}
