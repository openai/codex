use crate::CodexAuth;
use crate::config::Config;
use codex_capabilities::skills::RemoteSkillRequest;
use codex_capabilities::skills::remote::RemoteSkillDownloadResult;
use codex_capabilities::skills::remote::RemoteSkillProductSurface;
use codex_capabilities::skills::remote::RemoteSkillScope;
use codex_capabilities::skills::remote::RemoteSkillSummary;

pub async fn list_remote_skills(
    config: &Config,
    auth: Option<&CodexAuth>,
    scope: RemoteSkillScope,
    product_surface: RemoteSkillProductSurface,
    enabled: Option<bool>,
) -> anyhow::Result<Vec<RemoteSkillSummary>> {
    codex_capabilities::skills::remote::list_remote_skills(
        &RemoteSkillRequest {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            codex_home: config.codex_home.clone(),
        },
        auth,
        scope,
        product_surface,
        enabled,
    )
    .await
}

pub async fn export_remote_skill(
    config: &Config,
    auth: Option<&CodexAuth>,
    skill_id: &str,
) -> anyhow::Result<RemoteSkillDownloadResult> {
    codex_capabilities::skills::remote::export_remote_skill(
        &RemoteSkillRequest {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            codex_home: config.codex_home.clone(),
        },
        auth,
        skill_id,
    )
    .await
}
