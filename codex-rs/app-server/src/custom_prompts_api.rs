use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use codex_app_server_protocol::CustomPrompt;
use codex_app_server_protocol::CustomPromptsListParams;
use codex_app_server_protocol::CustomPromptsListResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_core::config_loader::LoaderOverrides;
use codex_core::config_loader::load_config_layers_state;
use codex_core::custom_prompts::default_prompts_dir;
use codex_core::custom_prompts::discover_layered_prompts_for_cwd;
use codex_core::custom_prompts::discover_prompts_in;
use codex_core::env;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::PathBuf;
use toml::Value as TomlValue;

#[derive(Clone)]
pub(crate) struct CustomPromptsApi {
    codex_home: PathBuf,
    cli_overrides: Vec<(String, TomlValue)>,
    loader_overrides: LoaderOverrides,
}

impl CustomPromptsApi {
    pub(crate) fn new(
        codex_home: PathBuf,
        cli_overrides: Vec<(String, TomlValue)>,
        loader_overrides: LoaderOverrides,
    ) -> Self {
        Self {
            codex_home,
            cli_overrides,
            loader_overrides,
        }
    }

    pub(crate) async fn list(
        &self,
        params: CustomPromptsListParams,
    ) -> Result<CustomPromptsListResponse, JSONRPCErrorError> {
        let prompts = if let Some(cwd) = params.cwd {
            let normalized = normalize_for_wsl(&cwd);
            let cwd_path = PathBuf::from(normalized);
            let cwd_abs = AbsolutePathBuf::from_absolute_path(&cwd_path).map_err(|err| {
                JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("cwd must be an absolute path: {err}"),
                    data: None,
                }
            })?;
            let layers = load_config_layers_state(
                &self.codex_home,
                Some(cwd_abs.clone()),
                &self.cli_overrides,
                self.loader_overrides.clone(),
            )
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to load config layers: {err}"),
                data: None,
            })?;
            discover_layered_prompts_for_cwd(cwd_abs.as_path(), &layers).await
        } else if let Some(dir) = default_prompts_dir() {
            discover_prompts_in(&dir).await
        } else {
            Vec::new()
        };

        Ok(CustomPromptsListResponse {
            custom_prompts: prompts.into_iter().map(map_custom_prompt).collect(),
        })
    }
}

fn map_custom_prompt(prompt: codex_protocol::custom_prompts::CustomPrompt) -> CustomPrompt {
    CustomPrompt {
        name: prompt.name,
        path: prompt.path,
        content: prompt.content,
        description: prompt.description,
        argument_hint: prompt.argument_hint,
    }
}

fn normalize_for_wsl(path: &str) -> String {
    if !env::is_wsl() {
        return path.to_string();
    }
    win_path_to_wsl(path).unwrap_or_else(|| path.to_string())
}

fn win_path_to_wsl(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    if bytes.len() < 3
        || bytes[1] != b':'
        || !(bytes[2] == b'\\' || bytes[2] == b'/')
        || !bytes[0].is_ascii_alphabetic()
    {
        return None;
    }
    let drive = (bytes[0] as char).to_ascii_lowercase();
    let tail = path[3..].replace('\\', "/");
    if tail.is_empty() {
        return Some(format!("/mnt/{drive}"));
    }
    Some(format!("/mnt/{drive}/{tail}"))
}
