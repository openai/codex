//! Read and save voice preferences with this app-server's ChatGPT account.

use super::*;
use codex_app_server_protocol::AccountVoiceOption;
use codex_app_server_protocol::AccountVoiceSettingsReadResponse;
use codex_app_server_protocol::AccountVoiceSettingsWriteParams;
use codex_app_server_protocol::AccountVoiceSettingsWriteResponse;

impl AccountRequestProcessor {
    async fn voice_settings_client(&self) -> Result<BackendClient, JSONRPCErrorError> {
        let (auth, factory) = self
            .auth_manager
            .auth_with_http_client_factory()
            .await
            .filter(|(auth, _)| auth.is_chatgpt_auth())
            .ok_or_else(|| invalid_request("ChatGPT authentication required for voice settings"))?;
        let config = self.load_latest_config().await;
        let url = config.chatgpt_backend_url();
        let root = url
            .resolve(&auth)
            .trim_end_matches('/')
            .trim_end_matches("/backend-api");
        let runtime = self
            .thread_manager
            .runtime_auth_for_config(&config)
            .map_err(|err| internal_error(err.to_string()))?;
        BackendClient::from_auth_with_runtime_without_redirects(
            format!("{root}/backend-api"),
            &auth,
            factory,
            runtime.as_ref(),
            &Default::default(),
        )
        .await
        .map_err(|err| internal_error(err.to_string()))
    }

    pub(crate) async fn read_voice_settings(
        &self,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let settings = self
            .voice_settings_client()
            .await?
            .get_voice_settings()
            .await
            .map_err(|err| internal_error(err.to_string()))?;
        let voices = settings
            .voices
            .into_iter()
            .map(|option| AccountVoiceOption {
                name: option.name.unwrap_or_else(|| option.voice.clone()),
                voice: option.voice,
            })
            .collect();
        Ok(Some(
            AccountVoiceSettingsReadResponse {
                selected: settings.selected,
                voices,
            }
            .into(),
        ))
    }

    pub(crate) async fn write_voice_settings(
        &self,
        params: AccountVoiceSettingsWriteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.voice_settings_client()
            .await?
            .set_voice(&params.voice)
            .await
            .map_err(|err| internal_error(err.to_string()))?;
        Ok(Some(AccountVoiceSettingsWriteResponse {}.into()))
    }
}
