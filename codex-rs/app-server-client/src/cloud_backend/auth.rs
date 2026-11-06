//! Opens the internal Cloud preview with the selected ChatGPT credentials and local transport policy.

use super::start_worker;
use crate::AppServerClient;
use crate::legacy_core::config::Config;
use codex_app_server_protocol as protocol;
use codex_cloud_client as cloud;
use codex_login::token_data::parse_jwt_expiration;
use codex_protocol::account::PlanType;
use color_eyre::eyre::Result;
use color_eyre::eyre::bail;
use color_eyre::eyre::eyre;

const ORIGIN: &str = "https://codex-cloud-backend.chatgpt.com";

pub async fn connect(config: &Config) -> Result<AppServerClient> {
    // The preview borrows Local's policy without owning credential updates.
    let factory = config.http_client_factory();
    let policy = factory.network_policy().clone().for_current_account();
    let factory = factory.with_network_policy(policy);
    let auth = config
        .auth_config()
        .load_auth(/*enable_codex_api_key_env*/ false)
        .await?
        .ok_or_else(|| eyre!("Sign in with ChatGPT and reopen Codex to use Cloud."))?;
    if !auth.is_chatgpt_auth() || auth.is_fedramp_account() {
        bail!("This account is not supported by the cloud conversation preview.");
    }
    let account_id = auth
        .get_account_id()
        .ok_or_else(|| eyre!("Cloud requires a selected account."))?;
    let token = auth.get_token()?;
    if let Ok(Some(expires)) = parse_jwt_expiration(&token)
        && std::time::SystemTime::from(expires) <= std::time::SystemTime::now()
    {
        bail!("Your ChatGPT credential expired. Reopen Codex to reconnect.");
    }
    factory.network_policy().acquire(&ORIGIN.parse()?)?;
    let credentials = cloud::Credentials::new(&token, &account_id)?;
    let client = cloud::Client::new(&factory, ORIGIN, credentials)?;
    Ok(start_worker(
        client,
        protocol::GetAccountResponse {
            user_id: auth.get_chatgpt_user_id(),
            account: Some(protocol::Account::Chatgpt {
                email: auth.get_account_email(),
                plan_type: auth.account_plan_type().unwrap_or(PlanType::Unknown),
                workspace_name: None,
            }),
            requires_openai_auth: true,
            workspace_routing: None,
        },
    ))
}
