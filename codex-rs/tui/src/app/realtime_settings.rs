//! Persist server voice preferences and machine-local audio selections before updating the UI.

use super::*;
use crate::legacy_core::config::edit::ConfigEdit;
use codex_app_server_protocol::AccountVoiceOption;
use codex_app_server_protocol::AccountVoiceSettingsWriteParams;
use codex_app_server_protocol::AccountVoiceSettingsWriteResponse;
use codex_config::config_toml::MicrophoneChannels;
use codex_realtime_webrtc::AudioDeviceKind;

const VOICE_SETTINGS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

async fn voice_settings_request<T: serde::de::DeserializeOwned>(
    handle: AppServerRequestHandle,
    request: ClientRequest,
) -> anyhow::Result<T> {
    Ok(tokio::time::timeout(VOICE_SETTINGS_TIMEOUT, handle.request_typed(request)).await??)
}

impl App {
    pub(super) async fn persist_realtime_input_channel(
        &mut self,
        channel: Option<MicrophoneChannels>,
    ) {
        self.persist_realtime_audio(
            "microphone_channel",
            channel.map(|channel| match channel {
                MicrophoneChannels::Single(channel) => i64::from(channel.get()).into(),
                MicrophoneChannels::Multiple(channels) => toml::Value::Array(
                    channels
                        .into_iter()
                        .map(|channel| i64::from(channel.get()).into())
                        .collect(),
                ),
            }),
        )
        .await;
    }

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
        let mut edits = vec![match &value {
            Some(toml::Value::String(value)) => ConfigEdit::SetPath {
                segments,
                value: value.clone().into(),
            },
            Some(toml::Value::Integer(value)) => ConfigEdit::SetPath {
                segments,
                value: (*value).into(),
            },
            Some(toml::Value::Array(channels)) => ConfigEdit::SetPath {
                segments,
                value: channels
                    .iter()
                    .map(|channel| match channel {
                        toml::Value::Integer(channel) => *channel,
                        _ => unreachable!("audio channel arrays contain only integers"),
                    })
                    .collect::<toml_edit::Array>()
                    .into(),
            },
            Some(_) => unreachable!("audio settings are device names or channel numbers"),
            None => ConfigEdit::ClearPath { segments },
        }];
        if key == "microphone" {
            edits.push(ConfigEdit::ClearPath {
                segments: vec!["audio".into(), "microphone_channel".into()],
            });
        }
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
                let channel_overridden = key == "microphone" && audio.microphone_channel.is_some();
                self.local_settings.audio = Ok(audio);
                self.chat_widget.local_settings.audio = self.local_settings.audio.clone();
                if channel_overridden {
                    self.chat_widget.add_error_message("Input device saved, but the input channel is overridden by another configuration layer. Update that override before starting voice.".into());
                } else if effective == value {
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

    pub(super) async fn ensure_voice_account(&mut self) -> bool {
        if let Err(error) = self.realtime_call_client.voice_provider(&self.config).await {
            self.chat_widget.add_error_message(error.to_string());
            return false;
        }
        true
    }

    pub(super) fn load_realtime_voices(&mut self, app_server: &mut AppServerSession) {
        self.voice_settings_pending = true;
        let origin = self.active_thread_id;
        let handle = app_server.request_handle();
        let request = ClientRequest::AccountVoiceSettingsRead {
            request_id: app_server.next_request_id(),
            params: None,
        };
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = voice_settings_request(handle, request).await;
            tx.send(AppEvent::RealtimeVoicesLoaded { origin, result });
        });
    }

    pub(super) fn persist_realtime_voice(
        &mut self,
        app_server: &mut AppServerSession,
        voice: AccountVoiceOption,
    ) {
        self.voice_settings_pending = true;
        let origin = self.active_thread_id;
        let handle = app_server.request_handle();
        let request = ClientRequest::AccountVoiceSettingsWrite {
            request_id: app_server.next_request_id(),
            params: AccountVoiceSettingsWriteParams { voice: voice.voice },
        };
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = voice_settings_request(handle, request)
                .await
                .map(|_: AccountVoiceSettingsWriteResponse| voice.name);
            tx.send(AppEvent::RealtimeVoiceSaved { origin, result });
        });
    }
}
