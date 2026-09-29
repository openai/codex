//! Derives the enterprise credential owner from the active Codex account.

use codex_config::McpEmaAuthScope;
use codex_login::CodexAuth;

pub fn ema_auth_scope(auth: Option<&CodexAuth>) -> Option<McpEmaAuthScope> {
    let auth = auth?;
    McpEmaAuthScope::new(auth.get_chatgpt_user_id()?, auth.get_account_id()?)
}
