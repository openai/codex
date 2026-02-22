mod auth_fixtures;
mod config;
mod mcp_process;
mod mock_model_server;
mod models_cache;
mod responses;
mod rollout;

pub use auth_fixtures::ChatGptAuthFixture;
pub use auth_fixtures::ChatGptIdTokenClaims;
pub use auth_fixtures::encode_id_token;
pub use auth_fixtures::write_chatgpt_auth;
use codex_app_server_protocol::JSONRPCResponse;
use codex_utils_cargo_bin::find_resource;
pub use config::write_mock_responses_config_toml;
pub use core_test_support::format_with_current_shell;
pub use core_test_support::format_with_current_shell_display;
pub use core_test_support::format_with_current_shell_display_non_login;
pub use core_test_support::format_with_current_shell_non_login;
pub use core_test_support::test_path_buf_with_windows;
pub use core_test_support::test_tmp_path;
pub use core_test_support::test_tmp_path_buf;
pub use mcp_process::DEFAULT_CLIENT_NAME;
pub use mcp_process::McpProcess;
pub use mock_model_server::create_mock_responses_server_repeating_assistant;
pub use mock_model_server::create_mock_responses_server_sequence;
pub use mock_model_server::create_mock_responses_server_sequence_unchecked;
pub use models_cache::write_models_cache;
pub use models_cache::write_models_cache_with_models;
pub use responses::create_apply_patch_sse_response;
pub use responses::create_exec_command_sse_response;
pub use responses::create_final_assistant_message_sse_response;
pub use responses::create_request_user_input_sse_response;
pub use responses::create_shell_command_sse_response;
pub use rollout::create_fake_rollout;
pub use rollout::create_fake_rollout_with_source;
pub use rollout::create_fake_rollout_with_text_elements;
pub use rollout::rollout_path;
use serde::de::DeserializeOwned;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Command;

pub fn to_response<T: DeserializeOwned>(response: JSONRPCResponse) -> anyhow::Result<T> {
    let value = serde_json::to_value(response.result)?;
    let codex_response = serde_json::from_value(value)?;
    Ok(codex_response)
}

pub struct TestZshExecutable {
    pub path: PathBuf,
    dotslash_cache_dir: Option<PathBuf>,
}

impl TestZshExecutable {
    pub async fn new_mcp_process(&self, codex_home: &Path) -> anyhow::Result<McpProcess> {
        if let Some(dotslash_cache_dir) = &self.dotslash_cache_dir {
            let dotslash_cache = dotslash_cache_dir.to_string_lossy().into_owned();
            return McpProcess::new_with_env(
                codex_home,
                &[("DOTSLASH_CACHE", Some(&dotslash_cache))],
            )
            .await;
        }

        McpProcess::new(codex_home).await
    }
}

pub async fn find_dotslash_test_zsh() -> anyhow::Result<TestZshExecutable> {
    let zsh = find_resource!("../suite/zsh")?;
    if !zsh.is_file() {
        anyhow::bail!("zsh fork test fixture not found: {}", zsh.display());
    }

    let dotslash_cache_dir = create_dotslash_cache_dir()?;
    let status = Command::new("dotslash")
        .arg("--")
        .arg("fetch")
        .arg(&zsh)
        .env("DOTSLASH_CACHE", &dotslash_cache_dir)
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("dotslash fetch failed for {}: {status}", zsh.display());
    }

    let zsh = TestZshExecutable {
        path: zsh,
        dotslash_cache_dir: Some(dotslash_cache_dir),
    };
    if !supports_exec_wrapper_intercept(&zsh) {
        anyhow::bail!(
            "zsh fork test fixture does not support EXEC_WRAPPER intercepts: {}",
            zsh.path.display()
        );
    }

    eprintln!("using zsh path for zsh-fork test: {}", zsh.path.display());

    Ok(zsh)
}

fn create_dotslash_cache_dir() -> anyhow::Result<PathBuf> {
    let timestamp_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let dotslash_cache_dir = std::env::temp_dir().join(format!(
        "codex-zsh-dotslash-cache-{}-{timestamp_nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dotslash_cache_dir)?;
    Ok(dotslash_cache_dir)
}

fn supports_exec_wrapper_intercept(zsh: &TestZshExecutable) -> bool {
    let mut cmd = std::process::Command::new(&zsh.path);
    cmd.arg("-fc")
        .arg("/usr/bin/true")
        .env("EXEC_WRAPPER", "/usr/bin/false");
    if let Some(dotslash_cache_dir) = &zsh.dotslash_cache_dir {
        cmd.env("DOTSLASH_CACHE", dotslash_cache_dir);
    }
    let status = cmd.status();
    match status {
        Ok(status) => !status.success(),
        Err(_) => false,
    }
}
