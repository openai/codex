//! Persist server voice preferences and machine-local audio selections before updating the UI.

use super::*;
use crate::legacy_core::config::edit::ConfigEdit;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadRealtimeListVoicesParams;
use codex_app_server_protocol::ThreadRealtimeListVoicesResponse;
use codex_protocol::protocol::RealtimeVoice;
use codex_protocol::protocol::RealtimeVoicesList;
use codex_realtime_webrtc::AudioDeviceKind;
use uuid::Uuid;

/// Unknown voices belong to newer servers; omit the override instead of blocking audio.
/// An unset preference explicitly selects the V1/V3 server default, not a stale thread preference.
fn configured_voice(
    config: &codex_app_server_protocol::Config,
    default_voice: RealtimeVoice,
) -> Result<Option<RealtimeVoice>> {
    let value = config
        .additional
        .get("realtime")
        .and_then(|realtime| realtime.get("voice"));
    match value {
        None | Some(serde_json::Value::Null) => Ok(Some(default_voice)),
        Some(value @ serde_json::Value::String(_)) => {
            Ok(serde_json::from_value(value.clone()).ok())
        }
        Some(value) => Ok(Some(serde_json::from_value(value.clone())?)),
    }
}

impl App {
    pub(super) async fn persist_realtime_device(
        &mut self,
        kind: AudioDeviceKind,
        name: Option<String>,
    ) {
        let key = match kind {
            AudioDeviceKind::Input => "microphone",
            AudioDeviceKind::Output => "speaker",
        };
        self.persist_realtime_audio(key, name.map(Into::into)).await;
        self.list_realtime_devices(kind);
    }

    async fn persist_realtime_audio(&mut self, key: &str, value: Option<toml::Value>) {
        // Audio runs on the TUI's machine, even when the app server is remote.
        let segments = vec!["audio".to_string(), key.to_string()];
        let edits = vec![match &value {
            Some(toml::Value::String(value)) => ConfigEdit::SetPath {
                segments,
                value: value.clone().into(),
            },
            Some(_) => unreachable!("audio device settings are names"),
            None => ConfigEdit::ClearPath { segments },
        }];
        if let Err(error) =
            ConfigEditsBuilder::for_config_path(self.local_settings.user_config_path.as_path())
                .with_edits(edits)
                .apply()
                .await
        {
            self.chat_widget
                .add_error_message(format!("Failed to save audio setting: {error}"));
            return;
        }
        match crate::legacy_core::config::load_config_toml_with_layer_stack(
            self.local_settings.codex_home.as_path(),
            /*cwd*/ None,
            self.cli_kv_overrides.clone(),
            codex_config::ConfigLoadOptions {
                loader_overrides: self.loader_overrides.clone(),
                cloud_config_bundle: self.cloud_config_bundle.clone(),
                ..Default::default()
            },
        )
        .await
        {
            Ok(config) => {
                self.config.config_layer_stack = self
                    .config
                    .config_layer_stack
                    .with_user_layer_from(&config.config_layer_stack);
                let audio = config.config_toml.audio.unwrap_or_default();
                let effective = toml::Value::try_from(&audio)
                    .ok()
                    .and_then(|audio| audio.get(key).cloned());
                self.local_settings.audio = Ok(audio);
                self.chat_widget.local_settings.audio = self.local_settings.audio.clone();
                if effective == value {
                    self.chat_widget.add_info_message(
                        "Audio setting saved. Applies to your next voice conversation.".to_string(),
                        /*hint*/ None,
                    );
                } else {
                    self.chat_widget.add_error_message(
                        "Audio setting was saved but is overridden by another configuration layer."
                            .to_string(),
                    );
                }
            }
            Err(error) => self.chat_widget.add_error_message(format!(
                "Audio setting was saved, but effective settings could not be read: {error}"
            )),
        }
    }

    pub(super) fn list_realtime_devices(&self, kind: AudioDeviceKind) {
        let tx = self.app_event_tx.clone();
        let origin = self.active_thread_id;
        tokio::spawn(async move {
            let result = async {
                let package = codex_install_context::InstallContext::current()
                    .package_layout
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("voice package unavailable"))?;
                let mut host = codex_realtime_webrtc::VoiceHost::connect(
                    package,
                    codex_build_info::BuildInfo::get().build_commit(),
                )
                .await?;
                let result = host.list_devices(kind).await;
                host.close().await?;
                result
            }
            .await;
            tx.send(AppEvent::RealtimeDevicesListed {
                origin,
                kind,
                result: result.map_err(|_| {
                    "Could not list audio devices. Check your audio device and voice package."
                        .to_owned()
                }),
            });
        });
    }

    pub(super) async fn realtime_voices(
        &self,
        app_server: &AppServerSession,
    ) -> Result<RealtimeVoicesList> {
        let response = app_server
            .request_handle()
            .request_typed::<ThreadRealtimeListVoicesResponse>(
                ClientRequest::ThreadRealtimeListVoices {
                    request_id: RequestId::String(format!("tui-voice-list-{}", Uuid::new_v4())),
                    params: ThreadRealtimeListVoicesParams {},
                },
            )
            .await?;
        Ok(response.voices)
    }

    pub(super) async fn effective_realtime_voice(
        &self,
        app_server: &AppServerSession,
        voices: &RealtimeVoicesList,
    ) -> Result<Option<RealtimeVoice>> {
        // Session setup installs the active conversation cwd for both server modes.
        let config = crate::config_update::read_effective_config_if_supported(
            app_server.request_handle(),
            &self.chat_widget.config_ref().cwd,
        )
        .await?;
        config
            .as_ref()
            .map(|config| configured_voice(&config.config, voices.default_v1))
            .transpose()
            .map(Option::flatten)
    }

    pub(super) async fn open_realtime_voices(&mut self, app_server: &AppServerSession) {
        let voices = match self.realtime_voices(app_server).await {
            Ok(voices) => voices,
            Err(error) => {
                self.chat_widget
                    .add_error_message(format!("Failed to list voices: {error}"));
                return;
            }
        };
        match self.effective_realtime_voice(app_server, &voices).await {
            Ok(voice) => self.chat_widget.open_realtime_voices(voice, voices),
            Err(error) => self
                .chat_widget
                .add_error_message(format!("Failed to read voice settings: {error}")),
        }
    }

    pub(super) async fn persist_realtime_voice(
        &mut self,
        app_server: &AppServerSession,
        voice: RealtimeVoice,
    ) {
        match crate::config_update::write_config_batch(
            app_server.request_handle(),
            vec![crate::config_update::replace_config_value(
                "realtime.voice",
                serde_json::json!(voice),
            )],
        )
        .await
        {
            Ok(response) => {
                let voices = match self.realtime_voices(app_server).await {
                    Ok(voices) => voices,
                    Err(error) => {
                        self.chat_widget.add_error_message(format!(
                            "Voice preference was saved, but the effective voice could not be confirmed: {error}"
                        ));
                        return;
                    }
                };
                let effective = crate::config_update::read_effective_config_if_supported(
                    app_server.request_handle(), &self.chat_widget.config_ref().cwd,
                ).await.and_then(|config| match config.as_ref() {
                    Some(config) => configured_voice(&config.config, voices.default_v1),
                    None if response.status == codex_app_server_protocol::WriteStatus::OkOverridden => Err(color_eyre::eyre::eyre!("the saved voice is overridden, but this server cannot read effective settings")),
                    None => Ok(Some(voice)),
                });
                match effective {
                    Ok(effective_voice) => {
                        self.config.realtime.voice = effective_voice;
                        self.chat_widget.set_realtime_voice(effective_voice);
                        if effective_voice == Some(voice) {
                            self.chat_widget.on_realtime_voice_saved(voice);
                        } else {
                            self.chat_widget.add_error_message(format!(
                                "Voice preference was saved but not applied: {}",
                                super::config_persistence::overridden_write_message(&response),
                            ));
                        }
                    }
                    Err(error) => self.chat_widget.add_error_message(format!("Voice preference was saved, but effective settings could not be read: {error}")),
                }
            }
            Err(error) => self
                .chat_widget
                .add_error_message(format!("Failed to save voice: {error}")),
        }
    }
}
