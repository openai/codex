use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crate::AuthManager;
use crate::ModelProviderInfo;
use crate::auth::CodexAuth;
use crate::client::ModelClient;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::config::Config;
use crate::error::CodexErr;
use crate::protocol::SandboxPolicy;
use crate::terminal;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SandboxCommandAssessment;
use futures::StreamExt;
use serde_json::json;
use tokio::time::timeout;
use tracing::debug;
use tracing::warn;

const SANDBOX_ASSESSMENT_SYSTEM_PROMPT: &str = r#"You are a security analyst evaluating shell commands that were blocked by a sandbox. Given the provided metadata, summarize the command's likely intent and assess the risk. Return strictly valid JSON with the keys:
- description (concise summary, at most two sentences)
- risk_level ("low", "medium", or "high")
- risk_categories (optional array of zero or more category strings)
Risk level examples:
- low: read-only inspections, listing files, printing configuration
- medium: modifying project files, installing dependencies, fetching artifacts from trusted sources
- high: deleting or overwriting data, exfiltrating secrets, escalating privileges, or disabling security controls
Recognized risk_categories: data_deletion, data_exfiltration, privilege_escalation, system_modification, network_access, resource_exhaustion, compliance.
Use multiple categories when appropriate.
Placeholders such as <workspace> or <home> indicate redacted sensitive paths.
If information is insufficient, choose the most cautious risk level supported by the evidence.
Respond with JSON only, without markdown code fences or extra commentary."#;

const SANDBOX_ASSESSMENT_TIMEOUT: Duration = Duration::from_secs(5);

const SANDBOX_RISK_CATEGORY_VALUES: &[&str] = &[
    "data_deletion",
    "data_exfiltration",
    "privilege_escalation",
    "system_modification",
    "network_access",
    "resource_exhaustion",
    "compliance",
];

#[allow(clippy::too_many_arguments)]
pub(crate) async fn assess_command(
    config: Arc<Config>,
    provider: ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    parent_otel: &OtelEventManager,
    call_id: &str,
    command: &[String],
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
    failure_message: Option<&str>,
) -> Option<SandboxCommandAssessment> {
    if !config.sandbox_command_assessment || command.is_empty() {
        return None;
    }

    let command_json = serde_json::to_string(command).unwrap_or_else(|_| "[]".to_string());
    let command_joined =
        shlex::try_join(command.iter().map(String::as_str)).unwrap_or_else(|_| command.join(" "));
    let failure = failure_message
        .map(str::trim)
        .filter(|msg| !msg.is_empty())
        .map(str::to_string);

    let cwd_str = cwd.to_string_lossy().to_string();
    let sandbox_summary = summarize_sandbox_policy(sandbox_policy);
    let mut roots = sandbox_roots_for_prompt(sandbox_policy, cwd);
    roots.sort();
    roots.dedup();

    let platform = std::env::consts::OS;
    let mut prompt_sections = Vec::new();
    prompt_sections.push(format!("Platform: {platform}"));
    prompt_sections.push(format!("Sandbox policy: {sandbox_summary}"));
    if !roots.is_empty() {
        let formatted = roots
            .iter()
            .map(|root| root.to_string_lossy())
            .collect::<Vec<_>>()
            .join(", ");
        prompt_sections.push(format!("Filesystem roots: {formatted}"));
    }
    prompt_sections.push(format!("Working directory: {cwd_str}"));
    prompt_sections.push(format!("Command argv: {command_json}"));
    prompt_sections.push(format!("Command (joined): {command_joined}"));
    if let Some(msg) = failure.as_ref() {
        prompt_sections.push(format!("Sandbox failure message: {msg}"));
    }
    let metadata = prompt_sections.join("\n");
    let user_prompt = format!("Command metadata:\n{metadata}");

    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: user_prompt }],
        }],
        tools: Vec::new(),
        parallel_tool_calls: false,
        base_instructions_override: Some(SANDBOX_ASSESSMENT_SYSTEM_PROMPT.to_string()),
        output_schema: Some(sandbox_assessment_schema()),
    };

    let auth_snapshot = auth_manager.auth();
    let conversation_id = ConversationId::new();
    let child_otel = OtelEventManager::new(
        conversation_id,
        config.model.as_str(),
        config.model_family.slug.as_str(),
        auth_snapshot.as_ref().and_then(CodexAuth::get_account_id),
        auth_snapshot
            .as_ref()
            .and_then(CodexAuth::get_account_email),
        auth_snapshot.as_ref().map(|a| a.mode),
        config.otel.log_user_prompt,
        terminal::user_agent(),
    );
    child_otel.conversation_starts(
        config.model_provider.name.as_str(),
        config.model_reasoning_effort,
        config.model_reasoning_summary,
        config.model_context_window,
        config.model_max_output_tokens,
        config.model_auto_compact_token_limit,
        config.approval_policy,
        config.sandbox_policy.clone(),
        config.mcp_servers.keys().map(String::as_str).collect(),
        config.active_profile.clone(),
    );

    let client = ModelClient::new(
        Arc::clone(&config),
        Some(auth_manager),
        child_otel,
        provider,
        config.model_reasoning_effort,
        config.model_reasoning_summary,
        conversation_id,
    );

    let start = Instant::now();
    let assessment_result = timeout(SANDBOX_ASSESSMENT_TIMEOUT, async move {
        let mut stream = client.stream(&prompt).await?;
        let mut last_json: Option<String> = None;
        while let Some(event) = stream.next().await {
            match event {
                Ok(ResponseEvent::OutputItemDone(item)) => {
                    if let Some(text) = response_item_text(&item) {
                        last_json = Some(text);
                    }
                }
                Ok(ResponseEvent::RateLimits(_)) => {}
                Ok(ResponseEvent::Completed { .. }) => break,
                Ok(_) => continue,
                Err(err) => return Err(err),
            }
        }
        Ok(last_json)
    })
    .await;
    let duration = start.elapsed();

    match assessment_result {
        Ok(Ok(Some(raw))) => {
            if let Some(json_slice) = extract_assessment_json(&raw) {
                match serde_json::from_str::<SandboxCommandAssessment>(json_slice) {
                    Ok(assessment) => {
                        parent_otel.sandbox_assessment(
                            call_id,
                            "success",
                            Some(assessment.risk_level),
                            &assessment.risk_categories,
                            duration,
                        );
                        return Some(assessment);
                    }
                    Err(err) => {
                        warn!("failed to parse sandbox assessment JSON: {err}");
                        parent_otel.sandbox_assessment(call_id, "parse_error", None, &[], duration);
                    }
                }
            } else {
                warn!("sandbox assessment response missing JSON object");
                parent_otel.sandbox_assessment(call_id, "parse_error", None, &[], duration);
            }
        }
        Ok(Ok(None)) => {
            warn!("sandbox assessment response did not include any message");
            parent_otel.sandbox_assessment(call_id, "no_output", None, &[], duration);
        }
        Ok(Err(err)) => {
            if let CodexErr::UnexpectedStatus(unexpected) = &err {
                debug!(
                    "sandbox assessment failed: {err} (status: {}, body: {})",
                    unexpected.status, unexpected.body
                );
            } else {
                debug!("sandbox assessment failed: {err}");
            }
            parent_otel.sandbox_assessment(call_id, "model_error", None, &[], duration);
        }
        Err(_) => {
            debug!("sandbox assessment timed out");
            parent_otel.sandbox_assessment(call_id, "timeout", None, &[], duration);
        }
    }

    None
}

fn summarize_sandbox_policy(policy: &SandboxPolicy) -> String {
    match policy {
        SandboxPolicy::DangerFullAccess => "danger-full-access".to_string(),
        SandboxPolicy::ReadOnly => "read-only".to_string(),
        SandboxPolicy::WorkspaceWrite { network_access, .. } => {
            let network = if *network_access {
                "network"
            } else {
                "no-network"
            };
            format!("workspace-write (network_access={network})")
        }
    }
}

fn sandbox_roots_for_prompt(policy: &SandboxPolicy, cwd: &Path) -> Vec<PathBuf> {
    let mut roots = vec![cwd.to_path_buf()];
    if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = policy {
        roots.extend(writable_roots.iter().cloned());
    }
    roots
}

fn sandbox_assessment_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["description", "risk_level", "risk_categories"],
        "properties": {
            "description": {
                "type": "string",
                "minLength": 1,
                "maxLength": 500
            },
            "risk_level": {
                "type": "string",
                "enum": ["low", "medium", "high"]
            },
            "risk_categories": {
                "type": "array",
                "items": {
                    "type": "string",
                    "enum": SANDBOX_RISK_CATEGORY_VALUES
                }
            }
        },
        "additionalProperties": false
    })
}

fn extract_assessment_json(raw: &str) -> Option<&str> {
    let mut slice = raw.trim();
    if let Some(stripped) = slice.strip_prefix("```json") {
        slice = stripped.trim_start();
    }
    if let Some(stripped) = slice.strip_prefix("```") {
        slice = stripped.trim_start();
    }
    if let Some(stripped) = slice.strip_suffix("```") {
        slice = stripped.trim_end();
    }
    let slice = slice.trim();
    if slice.starts_with('{') && slice.ends_with('}') {
        return Some(slice);
    }
    let start = slice.find('{')?;
    let end = slice.rfind('}')?;
    if end <= start {
        return None;
    }
    slice.get(start..=end)
}

fn response_item_text(item: &ResponseItem) -> Option<String> {
    match item {
        ResponseItem::Message { content, .. } => {
            let mut buffers: Vec<&str> = Vec::new();
            for segment in content {
                match segment {
                    ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                        if !text.is_empty() {
                            buffers.push(text);
                        }
                    }
                    ContentItem::InputImage { .. } => {}
                }
            }
            if buffers.is_empty() {
                None
            } else {
                Some(buffers.join("\n"))
            }
        }
        ResponseItem::FunctionCallOutput { output, .. } => Some(output.content.clone()),
        _ => None,
    }
}
