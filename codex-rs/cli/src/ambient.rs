use anyhow::Result;
use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::ModelProviderInfo;
use codex_core::chat_completions::stream_chat_completions;
use codex_core::client_common::Prompt;
use codex_core::client_common::ResponseEvent;
use codex_core::config::Config;
use codex_core::model_family;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use futures::StreamExt;
use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Debug, Parser)]
pub struct AmbientCommand {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,
}

pub async fn run_main(cmd: AmbientCommand) -> Result<()> {
    println!("Ambient agent started. Monitoring for changes... (Press Ctrl+C to stop)");

    let cli_overrides = cmd
        .config_overrides
        .parse_overrides()
        .map_err(|e| anyhow::anyhow!(e))?;
    let config = Config::load_with_cli_overrides(cli_overrides, Default::default())?;
    let client = reqwest::Client::new();
    let cwd = std::env::current_dir()?;

    loop {
        if let Err(e) = perform_ambient_check(&config, &client, &cwd).await {
            eprintln!("[{}] Error: {}", chrono::Local::now().to_rfc2822(), e);
        }
        thread::sleep(Duration::from_secs(10));
    }
}

use std::path::Path;

async fn perform_ambient_check(
    config: &Config,
    client: &reqwest::Client,
    cwd: &Path,
) -> Result<()> {
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(cwd)
        .output()?;

    if output.stdout.is_empty() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();

    println!(
        "[{}] Found {} changed file(s).",
        chrono::Local::now().to_rfc2822(),
        lines.len()
    );

    if let Some(first_line) = lines.first() {
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() == 2 {
            let file_path = parts[1];
            println!("Analyzing: {file_path}");

            let full_path = cwd.join(file_path);
            let content = fs::read_to_string(&full_path)
                .map_err(|e| anyhow::anyhow!("Failed to read file {}: {e}", full_path.display()))?;

            let model_family = model_family::find_family_for_model(&config.model)
                .ok_or_else(|| anyhow::anyhow!("Model family not found for: {}", config.model))?;

            let provider = config
                .model_providers
                .get(&config.model_provider_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("Provider not found: {}", config.model_provider_id)
                })?;

            let prompt_text = format!(
                "You are an ambient AI assistant. Briefly summarize the change in this file:\n\n---\n\n{content}"
            );

            let user_message = ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: prompt_text }],
            };

            let prompt = Prompt {
                input: vec![user_message],
                store: false,
                tools: vec![],
                base_instructions_override: None,
            };

            let stream_result =
                stream_chat_completions(&prompt, &model_family, client, provider).await;

            match stream_result {
                Ok(mut stream) => {
                    println!("AI Insight:");
                    while let Some(event) = stream.next().await {
                        match event {
                            Ok(ResponseEvent::OutputTextDelta(delta)) => {
                                print!("{delta}");
                            }
                            Ok(ResponseEvent::Completed { .. }) => {
                                println!("\n---");
                                break;
                            }
                            Err(e) => {
                                return Err(anyhow::anyhow!("Error processing stream: {e:?}"));
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to get AI insight: {e}"));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::BUILT_IN_OSS_MODEL_PROVIDER_ID;
    use codex_core::WireApi;
    use codex_core::config_types::History;
    use codex_core::config_types::ShellEnvironmentPolicy;
    use codex_core::config_types::Tui;
    use codex_core::config_types::UriBasedFileOpener;
    use codex_core::model_family::find_family_for_model;
    use codex_core::protocol::AskForApproval;
    use codex_core::protocol::SandboxPolicy;
    use codex_protocol::mcp_protocol::AuthMode;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    async fn setup_test_env() -> (Config, MockServer, tempfile::TempDir) {
        let server = MockServer::start().await;
        let dir = tempdir().unwrap();
        std::process::Command::new("git")
            .arg("init")
            .current_dir(dir.path())
            .output()
            .unwrap();

        let model = "gpt-3.5-turbo".to_string();
        let model_family = find_family_for_model(&model).unwrap();
        let provider_id = BUILT_IN_OSS_MODEL_PROVIDER_ID.to_string();

        let provider_info = ModelProviderInfo {
            name: "test-provider".to_string(),
            base_url: Some(server.uri()),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(1),
            stream_max_retries: Some(1),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
        };

        let config = Config {
            model: model.clone(),
            model_family,
            model_provider_id: provider_id.clone(),
            model_provider: provider_info.clone(),
            model_providers: HashMap::from([(provider_id, provider_info)]),
            model_context_window: None,
            model_max_output_tokens: None,
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: SandboxPolicy::ReadOnly,
            shell_environment_policy: ShellEnvironmentPolicy::default(),
            hide_agent_reasoning: false,
            show_raw_agent_reasoning: false,
            disable_response_storage: false,
            user_instructions: None,
            base_instructions: None,
            notify: None,
            cwd: PathBuf::new(),
            mcp_servers: HashMap::new(),
            project_doc_max_bytes: 0,
            codex_home: PathBuf::new(),
            history: History::default(),
            file_opener: UriBasedFileOpener::VsCode,
            tui: Tui::default(),
            codex_linux_sandbox_exe: None,
            model_reasoning_effort: Default::default(),
            model_reasoning_summary: Default::default(),
            model_verbosity: None,
            chatgpt_base_url: "".to_string(),
            experimental_resume: None,
            include_plan_tool: false,
            include_apply_patch_tool: false,
            tools_web_search_request: false,
            responses_originator_header: "".to_string(),
            preferred_auth_method: AuthMode::ChatGPT,
            use_experimental_streamable_shell_tool: false,
            include_view_image_tool: false,
            disable_paste_burst: false,
        };

        (config, server, dir)
    }

    #[tokio::test]
    async fn test_ambient_check_happy_path() {
        let (config, server, dir) = setup_test_env().await;
        let client = reqwest::Client::new();

        // Create a dummy file change
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello").unwrap();
        std::process::Command::new("git")
            .arg("add")
            .arg("test.txt")
            .current_dir(dir.path())
            .output()
            .unwrap();

        // Mock the AI server response
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"choices\": [{\"delta\": {\"content\": \"summary\"}}]}\n\ndata: [DONE]\n\n",
            ))
            .mount(&server)
            .await;

        let result = perform_ambient_check(&config, &client, dir.path()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ambient_check_api_error() {
        let (config, server, dir) = setup_test_env().await;
        let client = reqwest::Client::new();

        // Create a dummy file change
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello").unwrap();
        std::process::Command::new("git")
            .arg("add")
            .arg("test.txt")
            .current_dir(dir.path())
            .output()
            .unwrap();

        // Mock the AI server to return an error
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let result = perform_ambient_check(&config, &client, dir.path()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to get AI insight"));
    }
}
